use crate::api::CreateOrUpdateNavRequest;
use crate::cache::swap_tmp_nav_id_in_snapshot;
use crate::drafts::{
    get_due_unsynced_nav_drafts, get_due_unsynced_nav_meta_drafts, get_unsynced_nav_drafts,
    list_dirty_notes, mark_nav_meta_sync_failed, mark_nav_meta_synced, mark_nav_sync_failed,
    mark_nav_synced, swap_tmp_nav_id_in_drafts, touch_nav, touch_nav_meta, NavMetaDraft,
};
use crate::state::AppContext;
use crate::util::now_ms;
use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsCast;

/// Global, local-first sync controller for note nav drafts.
///
/// Responsibilities:
/// - local draft writes (localStorage)
/// - per-nav debounce autosave
/// - retry queue (retry_count/next_retry_ms)
/// - best-effort pagehide flush (beacon/keepalive-friendly)
///
/// Non-responsibilities:
/// - outline UI state (editing id, focus, etc.)
#[derive(Clone)]
pub(crate) struct NoteSyncController {
    app_state: AppContext,

    /// Connectivity state to backend API.
    backend_online: RwSignal<bool>,
    last_backend_error: RwSignal<Option<String>>,

    /// When offline, we still probe occasionally to detect recovery, but never spam requests.
    offline_next_probe_ms: RwSignal<i64>,

    /// Current route context (set by NotePage via tracked Effect).
    current_db_id: RwSignal<String>,
    current_note_id: RwSignal<String>,
    current_editing_nav_id: RwSignal<Option<String>>,

    /// Per-nav debounce timers.
    autosave_ms: i32,
    autosave_timers: Arc<Mutex<HashMap<String, i32>>>,

    /// Retry worker.
    retry_timer_id: RwSignal<Option<i32>>,
    retry_interval_ms: i32,

    /// Global listeners (keep handles alive).
    _online_handle: StoredValue<Option<WindowListenerHandle>>,
    _pagehide_handle: StoredValue<Option<WindowListenerHandle>>,
}

impl NoteSyncController {
    pub fn is_backend_online(&self) -> bool {
        self.backend_online.get_untracked()
    }

    #[allow(dead_code)]
    pub fn last_backend_error(&self) -> Option<String> {
        self.last_backend_error.get_untracked()
    }

    // (removed) string-based network error detection; use ApiErrorKind::Network

    pub(crate) fn mark_backend_online(&self) {
        self.backend_online.set(true);
        self.last_backend_error.set(None);
        self.offline_next_probe_ms.set(0);
    }

    // (removed) mark_backend_offline(&str); use mark_backend_offline_api(ApiError)

    pub(crate) fn mark_backend_offline_api(&self, e: &crate::api::ApiError) {
        if e.kind == crate::api::ApiErrorKind::Network {
            self.backend_online.set(false);
            self.last_backend_error.set(Some(e.to_string()));
        }
    }

    fn should_probe_offline(&self, now_ms: i64) -> bool {
        if self.backend_online.get_untracked() {
            return true;
        }

        let next = self.offline_next_probe_ms.get_untracked();
        if next == 0 || now_ms >= next {
            true
        } else {
            false
        }
    }

    fn schedule_next_offline_probe(&self, now_ms: i64) {
        // Conservative: one probe every 15s while offline.
        self.offline_next_probe_ms.set(now_ms + 15_000);
    }

    pub fn new(app_state: AppContext) -> Self {
        let backend_online = RwSignal::new(true);
        let last_backend_error = RwSignal::new(None);
        let offline_next_probe_ms = RwSignal::new(0);

        let current_db_id = RwSignal::new(String::new());
        let current_note_id = RwSignal::new(String::new());
        let current_editing_nav_id = RwSignal::new(None);

        let autosave_ms = 1200;
        let autosave_timers = Arc::new(Mutex::new(HashMap::new()));

        let retry_timer_id = RwSignal::new(None);
        let retry_interval_ms = 2000;

        // We'll fill these in start() so they can reference `self` via clones.
        let _online_handle = StoredValue::new(None);
        let _pagehide_handle = StoredValue::new(None);

        let s = Self {
            app_state,
            backend_online,
            last_backend_error,
            offline_next_probe_ms,
            current_db_id,
            current_note_id,
            current_editing_nav_id,
            autosave_ms,
            autosave_timers,
            retry_timer_id,
            retry_interval_ms,
            _online_handle,
            _pagehide_handle,
        };

        s.start_global_listeners();
        s.start_retry_worker();

        s
    }

    fn db_note_untracked(&self) -> Option<(String, String)> {
        let db = self.current_db_id.get_untracked();
        let note = self.current_note_id.get_untracked();
        if db.trim().is_empty() || note.trim().is_empty() {
            None
        } else {
            Some((db, note))
        }
    }

    /// Called by NotePage (tracked Effect) when route changes.
    pub fn set_route(&self, db_id: String, note_id: String) {
        self.current_db_id.set(db_id);
        self.current_note_id.set(note_id);
    }

    /// Called by OutlineEditor when editing nav changes.
    pub fn set_editing_nav(&self, nav_id: Option<String>) {
        self.current_editing_nav_id.set(nav_id);
    }

    /// Called by OutlineEditor on each input.
    pub fn on_nav_changed(&self, nav_id: &str, content: &str) {
        let Some((db_id, note_id)) = self.db_note_untracked() else {
            return;
        };

        touch_nav(&db_id, &note_id, nav_id, content);
        self.schedule_autosave(nav_id.to_string());
    }

    pub fn on_nav_meta_changed(&self, nav: &crate::models::Nav) {
        let Some((db_id, note_id)) = self.db_note_untracked() else {
            return;
        };

        touch_nav_meta(&db_id, &note_id, nav);
        self.schedule_autosave(format!("meta:{}", nav.id));
    }

    fn flush_nav_draft(&self, nav_id: String) {
        // Never spam backend when offline; rely on retry worker probes.
        if !self.backend_online.get_untracked() {
            return;
        }

        let Some((db_id, note_id)) = self.db_note_untracked() else {
            return;
        };
        if nav_id.trim().is_empty() {
            return;
        }

        // tmp-* ids are optimistic local nodes. They must NOT be upserted by id;
        // they will be created via meta-draft (id=None) and then swapped to a real id.
        if nav_id.starts_with("tmp-") {
            return;
        }

        // meta:{nav_id} is used for metadata autosave.
        if let Some(id) = nav_id.strip_prefix("meta:") {
            self.flush_nav_meta_draft(id.to_string());
            return;
        }

        // Source of truth: local drafts.
        let Some((_, content, updated_ms)) = get_unsynced_nav_drafts(&db_id, &note_id)
            .into_iter()
            .find(|(id, _, _)| id == &nav_id)
        else {
            return;
        };

        let api_client = self.app_state.0.api_client.get_untracked();
        let s2 = self.clone();
        spawn_local(async move {
            let req = CreateOrUpdateNavRequest {
                note_id: note_id.clone(),
                id: Some(nav_id.clone()),
                parid: None,
                content: Some(content),
                order: None,
                is_display: None,
                is_delete: None,
                properties: None,
            };

            match api_client.upsert_nav(req).await {
                Ok(_) => {
                    s2.mark_backend_online();
                    mark_nav_synced(&db_id, &note_id, &nav_id, updated_ms);
                }
                Err(e) => {
                    s2.mark_backend_offline_api(&e);
                    mark_nav_sync_failed(&db_id, &note_id, &nav_id);
                }
            }
        });
    }

    fn flush_nav_meta_draft(&self, nav_id: String) {
        // Never spam backend when offline; rely on retry worker probes.
        if !self.backend_online.get_untracked() {
            return;
        }

        let Some((db_id, note_id)) = self.db_note_untracked() else {
            return;
        };
        if nav_id.trim().is_empty() {
            return;
        }

        // tmp-* ids must be created with id=None (see retry worker). Never upsert them by id.
        if nav_id.starts_with("tmp-") {
            return;
        }

        let Some((_, meta, updated_ms)) =
            get_due_unsynced_nav_meta_drafts(&db_id, &note_id, now_ms(), 50)
                .into_iter()
                .find(|(id, _, _)| id == &nav_id)
        else {
            return;
        };

        let api_client = self.app_state.0.api_client.get_untracked();
        let s2 = self.clone();
        spawn_local(async move {
            let req = CreateOrUpdateNavRequest {
                note_id: note_id.clone(),
                id: Some(nav_id.clone()),
                parid: Some(meta.parid),
                content: None,
                order: Some(meta.same_deep_order),
                is_display: Some(meta.is_display),
                is_delete: Some(meta.is_delete),
                properties: meta.properties,
            };

            match api_client.upsert_nav(req).await {
                Ok(_) => {
                    s2.mark_backend_online();
                    mark_nav_meta_synced(&db_id, &note_id, &nav_id, updated_ms);
                }
                Err(e) => {
                    s2.mark_backend_offline_api(&e);
                    mark_nav_meta_sync_failed(&db_id, &note_id, &nav_id);
                }
            }
        });
    }

    fn schedule_autosave(&self, nav_id: String) {
        if nav_id.trim().is_empty() {
            return;
        }

        let Some(win) = web_sys::window() else {
            return;
        };

        if let Ok(mut map) = self.autosave_timers.lock() {
            if let Some(tid) = map.remove(&nav_id) {
                let _ = win.clear_timeout_with_handle(tid);
            }
        }

        let s2 = self.clone();
        let nav_id2 = nav_id.clone();
        let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
            s2.flush_nav_draft(nav_id2);
        });

        let tid = win
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                self.autosave_ms,
            )
            .unwrap_or(0);

        if let Ok(mut map) = self.autosave_timers.lock() {
            map.insert(nav_id, tid);
        }
    }

    fn retry_tick(&self) {
        // Global retry: pick a few dirty notes and flush due items.
        let now = now_ms();

        if !self.should_probe_offline(now) {
            return;
        }

        if !self.backend_online.get_untracked() {
            self.schedule_next_offline_probe(now);
        }
        let candidates = list_dirty_notes(3);
        if candidates.is_empty() {
            return;
        }

        // Limit work per tick.
        let mut picked_content: Vec<(String, String, String, String, i64)> = vec![]; // db, note, nav, content, updated
        let mut picked_meta: Vec<(String, String, String, NavMetaDraft, i64)> = vec![]; // db, note, nav, meta, updated

        for (db_id, note_id) in candidates.into_iter() {
            // content
            let due_c = get_due_unsynced_nav_drafts(&db_id, &note_id, now, 2);
            for (nav_id, content, updated_ms) in due_c {
                picked_content.push((db_id.clone(), note_id.clone(), nav_id, content, updated_ms));
                if picked_content.len() + picked_meta.len() >= 2 {
                    break;
                }
            }

            if picked_content.len() + picked_meta.len() >= 2 {
                break;
            }

            // meta
            let due_m = get_due_unsynced_nav_meta_drafts(&db_id, &note_id, now, 2);
            for (nav_id, meta, updated_ms) in due_m {
                picked_meta.push((db_id.clone(), note_id.clone(), nav_id, meta, updated_ms));
                if picked_content.len() + picked_meta.len() >= 2 {
                    break;
                }
            }

            if picked_content.len() + picked_meta.len() >= 2 {
                break;
            }
        }

        if picked_content.is_empty() && picked_meta.is_empty() {
            return;
        }

        let api_client = self.app_state.0.api_client.get_untracked();
        let s2 = self.clone();
        spawn_local(async move {
            // 1) Handle pending creates (tmp nav ids) first.
            //    Strategy: create with id=None using meta draft; then swap tmp->real in snapshot+drafts.
            for (db_id, note_id, nav_id, meta, updated_ms) in picked_meta.iter() {
                if !nav_id.starts_with("tmp-") {
                    continue;
                }

                let req = CreateOrUpdateNavRequest {
                    note_id: note_id.clone(),
                    id: None,
                    parid: Some(meta.parid.clone()),
                    content: Some("".to_string()),
                    order: Some(meta.same_deep_order),
                    is_display: Some(meta.is_display),
                    is_delete: Some(meta.is_delete),
                    properties: meta.properties.clone(),
                };

                match api_client.upsert_nav(req).await {
                    Ok(resp) => {
                        s2.mark_backend_online();
                        let new_id = resp
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if new_id.trim().is_empty() {
                            continue;
                        }

                        swap_tmp_nav_id_in_drafts(db_id, note_id, nav_id, &new_id);
                        swap_tmp_nav_id_in_snapshot(db_id, note_id, nav_id, &new_id);

                        // Mark meta as synced under the real id.
                        mark_nav_meta_synced(db_id, note_id, &new_id, *updated_ms);
                    }
                    Err(e) => {
                        s2.mark_backend_offline_api(&e);
                        mark_nav_meta_sync_failed(db_id, note_id, nav_id);
                    }
                }
            }

            // 2) Sync content drafts (skip tmp ids; they will be backfilled after create).
            for (db_id, note_id, nav_id, content, updated_ms) in picked_content {
                if nav_id.starts_with("tmp-") {
                    continue;
                }

                let req = CreateOrUpdateNavRequest {
                    note_id: note_id.clone(),
                    id: Some(nav_id.clone()),
                    parid: None,
                    content: Some(content),
                    order: None,
                    is_display: None,
                    is_delete: None,
                    properties: None,
                };

                match api_client.upsert_nav(req).await {
                    Ok(_) => {
                        s2.mark_backend_online();
                        mark_nav_synced(&db_id, &note_id, &nav_id, updated_ms);
                    }
                    Err(e) => {
                        s2.mark_backend_offline_api(&e);
                        mark_nav_sync_failed(&db_id, &note_id, &nav_id);
                    }
                }
            }

            // 3) Sync meta drafts (non-tmp updates).
            for (db_id, note_id, nav_id, meta, updated_ms) in picked_meta {
                if nav_id.starts_with("tmp-") {
                    continue;
                }

                let req = CreateOrUpdateNavRequest {
                    note_id: note_id.clone(),
                    id: Some(nav_id.clone()),
                    parid: Some(meta.parid),
                    content: None,
                    order: Some(meta.same_deep_order),
                    is_display: Some(meta.is_display),
                    is_delete: Some(meta.is_delete),
                    properties: meta.properties,
                };

                match api_client.upsert_nav(req).await {
                    Ok(_) => {
                        s2.mark_backend_online();
                        mark_nav_meta_synced(&db_id, &note_id, &nav_id, updated_ms);
                    }
                    Err(e) => {
                        s2.mark_backend_offline_api(&e);
                        mark_nav_meta_sync_failed(&db_id, &note_id, &nav_id);
                    }
                }
            }
        });
    }

    fn start_retry_worker(&self) {
        if self.retry_timer_id.get_untracked().is_some() {
            return;
        }
        let Some(win) = web_sys::window() else {
            return;
        };

        let s2 = self.clone();
        let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
            s2.retry_tick();
        }) as Box<dyn FnMut()>);

        let tid = win
            .set_interval_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                self.retry_interval_ms,
            )
            .unwrap_or(0);
        self.retry_timer_id.set(Some(tid));

        // Global controller lives for app lifetime; no on_cleanup needed.
        cb.forget();
    }

    fn start_global_listeners(&self) {
        // online -> kick retry
        let s2 = self.clone();
        let online = window_event_listener(ev::online, move |_ev: web_sys::Event| {
            s2.retry_tick();
        });
        self._online_handle.set_value(Some(online));

        // pagehide -> flush current editing + recent K
        let s3 = self.clone();
        let pagehide =
            window_event_listener(ev::pagehide, move |_ev: web_sys::PageTransitionEvent| {
                s3.pagehide_flush();
            });
        self._pagehide_handle.set_value(Some(pagehide));
    }

    fn pagehide_flush(&self) {
        // When offline, pagehide flush would just spam failures.
        if !self.backend_online.get_untracked() {
            return;
        }

        let Some((db_id, note_id)) = self.db_note_untracked() else {
            return;
        };

        let mut drafts = get_unsynced_nav_drafts(&db_id, &note_id);
        if drafts.is_empty() {
            return;
        }
        drafts.sort_by(|a, b| b.2.cmp(&a.2));

        let k_recent: usize = 5;
        let mut picked: Vec<(String, String, i64)> = Vec::new();

        if let Some(editing_nav) = self.current_editing_nav_id.get_untracked() {
            if let Some(d) = drafts.iter().find(|(id, _, _)| id == &editing_nav) {
                picked.push(d.clone());
            }
        }

        for d in drafts.into_iter() {
            if picked.iter().any(|(id, _, _)| id == &d.0) {
                continue;
            }
            picked.push(d);
            if picked.len() >= k_recent {
                break;
            }
        }

        let api_client = self.app_state.0.api_client.get_untracked();
        let s2 = self.clone();
        spawn_local(async move {
            for (nav_id, content, updated_ms) in picked {
                if nav_id.starts_with("tmp-") {
                    continue;
                }

                let req = CreateOrUpdateNavRequest {
                    note_id: note_id.clone(),
                    id: Some(nav_id.clone()),
                    parid: None,
                    content: Some(content),
                    order: None,
                    is_display: None,
                    is_delete: None,
                    properties: None,
                };

                match api_client.upsert_nav(req).await {
                    Ok(_) => {
                        s2.mark_backend_online();
                        mark_nav_synced(&db_id, &note_id, &nav_id, updated_ms);
                    }
                    Err(e) => {
                        s2.mark_backend_offline_api(&e);
                        mark_nav_sync_failed(&db_id, &note_id, &nav_id);
                    }
                }
            }
        });
    }
}

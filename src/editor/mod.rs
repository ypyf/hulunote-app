use crate::api::CreateOrUpdateNavRequest;
use crate::models::{Nav, Note};
use crate::state::AppContext;
use crate::wiki::{extract_wiki_links, normalize_roam_page_title, parse_wiki_tokens, WikiToken};
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq, Eq)]
struct AcItem {
    title: String,
    is_new: bool,
}

#[derive(Clone)]
struct AutocompleteCtx {
    ac_open: RwSignal<bool>,
    ac_query: RwSignal<String>,
    ac_items: RwSignal<Vec<AcItem>>,
    ac_index: RwSignal<usize>,
    // Start position (UTF-16 code units) of the `[[` trigger in the current input.
    ac_start_utf16: RwSignal<Option<u32>>,

    // Cache all possible page titles for current DB (notes + wiki links from all navs).
    titles_cache_db: RwSignal<Option<String>>,
    titles_cache: RwSignal<Vec<String>>,
    titles_loading: RwSignal<bool>,
}

/// Update a nav's content in the local in-memory list.
///
/// This is used by multiple interaction paths (blur-save, click-to-switch, key navigation)
/// to avoid regressions where an edit buffer is lost during focus/unmount transitions.
pub(crate) fn apply_nav_content(navs: &mut [Nav], nav_id: &str, content: &str) -> bool {
    if let Some(n) = navs.iter_mut().find(|n| n.id == nav_id) {
        n.content = content.to_string();
        true
    } else {
        false
    }
}

pub(crate) fn is_tmp_nav_id(id: &str) -> bool {
    id.starts_with("tmp-")
}

fn utf16_to_byte_idx(s: &str, pos_utf16: u32) -> usize {
    if pos_utf16 == 0 {
        return 0;
    }
    let mut acc: u32 = 0;
    for (i, ch) in s.char_indices() {
        let w = ch.len_utf16() as u32;
        if acc + w > pos_utf16 {
            return i;
        }
        acc += w;
        if acc == pos_utf16 {
            return i + ch.len_utf8();
        }
    }
    s.len()
}

fn byte_idx_to_utf16(s: &str, byte_idx: usize) -> u32 {
    s[..byte_idx.min(s.len())].encode_utf16().count() as u32
}

fn ensure_titles_loaded(app_state: &AppContext, ac: &AutocompleteCtx) {
    let db_id = app_state
        .0
        .current_database_id
        .get_untracked()
        .unwrap_or_default();
    if db_id.trim().is_empty() {
        return;
    }

    if ac.titles_loading.get_untracked() {
        return;
    }

    if ac.titles_cache_db.get_untracked().as_deref() == Some(db_id.as_str())
        && !ac.titles_cache.get_untracked().is_empty()
    {
        return;
    }

    ac.titles_loading.set(true);
    ac.titles_cache_db.set(Some(db_id.clone()));

    let api_client = app_state.0.api_client.get_untracked();
    let notes = app_state.0.notes.get_untracked();

    let ac2 = ac.clone();
    spawn_local(async move {
        // 1) Existing note titles
        let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for n in notes {
            if n.database_id == db_id && !n.title.trim().is_empty() {
                set.insert(n.title);
            }
        }

        // 2) Titles referenced via [[...]] across all navs in DB (includes unreferenced pages).
        if let Ok(all_navs) = api_client.get_all_navs(&db_id).await {
            for nav in all_navs {
                if nav.is_delete {
                    continue;
                }
                for t in extract_wiki_links(&nav.content) {
                    if !t.trim().is_empty() {
                        set.insert(t);
                    }
                }
            }
        }

        ac2.titles_cache.set(set.into_iter().collect::<Vec<_>>());
        ac2.titles_loading.set(false);
    });
}

fn build_ac_items(titles: &[String], q: &str) -> Vec<AcItem> {
    let q_norm = q.to_lowercase();
    let mut items: Vec<AcItem> = vec![];

    // Create-new option (only if query is non-empty and not an exact existing title).
    let exact_exists = titles.iter().any(|t| t == q);
    if !q.trim().is_empty() && !exact_exists {
        items.push(AcItem {
            title: q.to_string(),
            is_new: true,
        });
    }

    // Existing titles (filter).
    for t in titles.iter().cloned() {
        if q_norm.trim().is_empty() || t.to_lowercase().contains(&q_norm) {
            // Avoid duplicating the create-new entry.
            if t == q {
                continue;
            }
            items.push(AcItem {
                title: t,
                is_new: false,
            });
        }
        if items.len() >= 20 {
            break;
        }
    }

    items
}

pub(crate) fn make_tmp_nav_id(now_ms: u64, rand: u64) -> String {
    format!("tmp-{now_ms}-{rand}")
}

pub(crate) fn swap_tmp_nav_id(navs: &mut [Nav], tmp_id: &str, real_id: &str) -> bool {
    if let Some(n) = navs.iter_mut().find(|n| n.id == tmp_id) {
        n.id = real_id.to_string();
        true
    } else {
        false
    }
}

pub(crate) fn get_nav_content(navs: &[Nav], nav_id: &str) -> Option<String> {
    navs.iter()
        .find(|n| n.id == nav_id)
        .map(|n| n.content.clone())
}

pub(crate) fn backfill_content_request(
    note_id: &str,
    real_id: &str,
    content_now: &str,
) -> Option<CreateOrUpdateNavRequest> {
    if content_now.trim().is_empty() {
        return None;
    }

    Some(CreateOrUpdateNavRequest {
        note_id: note_id.to_string(),
        id: Some(real_id.to_string()),
        parid: None,
        content: Some(content_now.to_string()),
        order: None,
        is_display: None,
        is_delete: None,
        properties: None,
    })
}

pub(crate) fn compute_reorder_target(
    all: &[Nav],
    dragged_id: &str,
    target_id: &str,
    insert_after: bool,
) -> Option<(String, f32)> {
    if dragged_id == target_id {
        return None;
    }

    let dragged = all.iter().find(|n| n.id == dragged_id)?;
    let target = all.iter().find(|n| n.id == target_id)?;

    let new_parid = target.parid.clone();

    // Build siblings in target parent, excluding dragged node (since it will move).
    let mut sibs = all
        .iter()
        .filter(|n| n.parid == new_parid && n.id != dragged_id)
        .cloned()
        .collect::<Vec<_>>();
    sibs.sort_by(|a, b| {
        a.same_deep_order
            .partial_cmp(&b.same_deep_order)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Find insertion index relative to target.
    let tidx = sibs.iter().position(|n| n.id == target_id)?;
    let insert_idx = if insert_after { tidx + 1 } else { tidx };

    // Determine prev/next order bounds.
    let prev_order = if insert_idx == 0 {
        None
    } else {
        Some(sibs[insert_idx - 1].same_deep_order)
    };

    let next_order = if insert_idx >= sibs.len() {
        None
    } else {
        Some(sibs[insert_idx].same_deep_order)
    };

    let new_order = match (prev_order, next_order) {
        (Some(p), Some(n)) => (p + n) / 2.0,
        (Some(p), None) => p + 1.0,
        (None, Some(n)) => n - 1.0,
        (None, None) => 0.0,
    };

    // No-op move detection: if staying in same parent and order is effectively unchanged, skip.
    if dragged.parid == new_parid && (dragged.same_deep_order - new_order).abs() < f32::EPSILON {
        return None;
    }

    Some((new_parid, new_order))
}

#[component]
pub fn OutlineEditor(
    note_id: impl Fn() -> String + Clone + Send + Sync + 'static,
    focused_nav_id: RwSignal<Option<String>>,
) -> impl IntoView {
    let app_state = expect_context::<AppContext>();

    let navs: RwSignal<Vec<Nav>> = RwSignal::new(vec![]);
    let loading: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);

    // Wiki links: opening a missing page does not hit the backend (Roam-style navigation).

    // Editing state
    let editing_id: RwSignal<Option<String>> = RwSignal::new(None);
    let editing_value: RwSignal<String> = RwSignal::new(String::new());
    let target_cursor_col: RwSignal<Option<u32>> = RwSignal::new(None);
    let editing_ref: NodeRef<html::Input> = NodeRef::new();

    // Autocomplete for `[[...]]` (Roam-style)
    // - Data source is fixed: existing notes + titles extracted from all nav contents in current DB.
    // - Supports creating new titles (insert text even if no existing note).
    let ac_open: RwSignal<bool> = RwSignal::new(false);
    let ac_query: RwSignal<String> = RwSignal::new(String::new());
    let ac_items: RwSignal<Vec<AcItem>> = RwSignal::new(vec![]);
    let ac_index: RwSignal<usize> = RwSignal::new(0);
    // Start position (UTF-16 code units) of the `[[` trigger in the current input.
    let ac_start_utf16: RwSignal<Option<u32>> = RwSignal::new(None);

    // Cache all possible page titles for current DB (notes + wiki links from all navs).
    let titles_cache_db: RwSignal<Option<String>> = RwSignal::new(None);
    let titles_cache: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let titles_loading: RwSignal<bool> = RwSignal::new(false);

    // Autocomplete recompute effect.
    // This fixes the first-`[[` case where titles are still loading: we keep the menu open and
    // populate items as soon as the async title load completes (without requiring extra typing).
    Effect::new(move |_| {
        let start = ac_start_utf16.get();
        if start.is_none() {
            return;
        }

        let q = ac_query.get();
        let loading_now = titles_loading.get();
        let titles_now = titles_cache.get();

        if loading_now {
            ac_open.set(true);
            // Keep items empty; UI will show a loading row.
            return;
        }

        let items = build_ac_items(&titles_now, &q);
        if items.is_empty() {
            ac_open.set(false);
            ac_index.set(0);
            return;
        }

        ac_items.set(items);
        ac_index.set(0);
        ac_open.set(true);
    });

    // Load navs when note_id changes.
    let note_id_for_effect = note_id.clone();
    Effect::new(move |_| {
        let id = note_id_for_effect();
        if id.trim().is_empty() {
            navs.set(vec![]);
            return;
        }

        loading.set(true);
        error.set(None);

        let api_client = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            match api_client.get_note_navs(&id).await {
                Ok(list) => navs.set(list),
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    });

    // Focus the inline editor when editing_id changes.
    Effect::new(move |_| {
        let id = editing_id.get();
        if id.is_none() {
            return;
        }

        let col = target_cursor_col.get_untracked();
        let el = editing_ref.get();
        if let Some(el) = el {
            // Focus on next tick so the node is mounted.
            let _ = web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    wasm_bindgen::closure::Closure::once_into_js(move || {
                        let _ = el.focus();
                        if let Some(col) = col {
                            // selectionStart/End are in UTF-16 code units.
                            let len = el.value().encode_utf16().count() as u32;
                            let pos = col.min(len);
                            let _ = el.set_selection_range(pos, pos);
                        }
                    })
                    .as_ref()
                    .unchecked_ref(),
                    0,
                );
        }
    });

    // Provide autocomplete context to OutlineNode.
    provide_context(AutocompleteCtx {
        ac_open,
        ac_query,
        ac_items,
        ac_index,
        ac_start_utf16,
        titles_cache_db,
        titles_cache,
        titles_loading,
    });

    view! {
        <div class="rounded-md border bg-card p-3">
            <div class="text-xs text-muted-foreground">"Outline"</div>

            // NOTE: intentionally no loading spinner when switching notes.

            <Show when=move || error.get().is_some() fallback=|| ().into_view()>
                {move || error.get().map(|e| view! {
                    <div class="mt-2 text-xs text-destructive">{e}</div>
                })}
            </Show>

            // (Roam-style) opening missing pages does not show an error banner here.

            <div class=move || {
                if editing_id.get().is_some() {
                    "mt-2 outline-editor outline-editor--editing"
                } else {
                    "mt-2 outline-editor"
                }
            }>
                {move || {
                    let all = navs.get();
                    let root = "00000000-0000-0000-0000-000000000000";

                    let mut roots = all
                        .iter()
                        .filter(|n| n.parid == root)
                        .cloned()
                        .collect::<Vec<_>>();
                    roots.sort_by(|a, b| a
                        .same_deep_order
                        .partial_cmp(&b.same_deep_order)
                        .unwrap_or(std::cmp::Ordering::Equal));

                    if roots.is_empty() {
                        view! { <div class="text-xs text-muted-foreground">"No nodes"</div> }
                            .into_any()
                    } else {
                        let nid_sv = StoredValue::new(note_id());
                        let root_ids_sv = StoredValue::new(
                            roots.into_iter().map(|n| n.id).collect::<Vec<String>>(),
                        );

                        view! {
                            <div class="space-y-0.5">
                                <For
                                    each=move || root_ids_sv.get_value()
                                    key=|id| id.clone()
                                    children=move |id| {
                                        let nid = nid_sv.get_value();
                                        view! {
                                            <OutlineNode
                                                nav_id=id
                                                depth=0
                                                navs=navs
                                                note_id=nid
                                                editing_id=editing_id
                                                editing_value=editing_value
                                                target_cursor_col=target_cursor_col
                                                editing_ref=editing_ref
                                                focused_nav_id=focused_nav_id
                                            />
                                        }
                                    }
                                />
                            </div>
                        }
                        .into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
pub fn OutlineNode(
    nav_id: String,
    depth: usize,
    navs: RwSignal<Vec<Nav>>,
    note_id: String,
    editing_id: RwSignal<Option<String>>,
    editing_value: RwSignal<String>,
    target_cursor_col: RwSignal<Option<u32>>,
    editing_ref: NodeRef<html::Input>,
    focused_nav_id: RwSignal<Option<String>>,
) -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let ac = expect_context::<AutocompleteCtx>();
    let navigate = leptos_router::hooks::use_navigate();

    let nav_id_for_nav = nav_id.clone();
    let nav_id_for_toggle = nav_id.clone();
    let nav_id_for_render = nav_id.clone();
    let note_id_for_toggle = note_id.clone();

    // (handler ids are captured per-render; avoid moving values out of the render closure)

    let nav_id_sv = StoredValue::new(nav_id.clone());
    let note_id_sv = StoredValue::new(note_id.clone());
    let app_state_sv = StoredValue::new(app_state.clone());
    let ac_sv = StoredValue::new(ac.clone());
    let navigate_sv = StoredValue::new(navigate.clone());

    let nav = move || navs.get().into_iter().find(|n| n.id == nav_id_for_nav);

    let on_toggle = Callback::new(move |_| {
        let Some(n) = navs
            .get_untracked()
            .into_iter()
            .find(|n| n.id == nav_id_for_toggle)
        else {
            return;
        };

        let new_display = !n.is_display;
        navs.update(|xs| {
            if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_for_toggle) {
                x.is_display = new_display;
            }
        });

        let api_client = app_state.0.api_client.get_untracked();
        let req = CreateOrUpdateNavRequest {
            note_id: note_id_for_toggle.clone(),
            id: Some(nav_id_for_toggle.clone()),
            parid: None,
            content: None,
            order: None,
            is_display: Some(new_display),
            is_delete: None,
            properties: None,
        };
        spawn_local(async move {
            let _ = api_client.upsert_nav(req).await;
        });
    });

    let indent_px = (depth * 18) as i32;

    view! {
        <div>
            {move || {
                let Some(n) = nav() else {
                    return ().into_view().into_any();
                };

                // Compute children for this render.
                let mut kids = navs
                    .get()
                    .into_iter()
                    .filter(|x| x.parid == nav_id_for_render)
                    .collect::<Vec<_>>();
                kids.sort_by(|a, b| {
                    a.same_deep_order
                        .partial_cmp(&b.same_deep_order)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let has_kids = !kids.is_empty();
                let (bullet, bullet_class) = if has_kids {
                    (
                        if n.is_display { "▾" } else { "▸" },
                        // Bigger + clearly clickable
                        "mt-0.5 h-5 w-5 text-base leading-none text-muted-foreground cursor-pointer hover:text-foreground/80",
                    )
                } else {
                    // Make leaf bullets more visible than a tiny middle dot.
                    ("•", "mt-0.5 h-5 w-5 text-base leading-none text-muted-foreground")
                };

                let on_toggle_cb = on_toggle.clone();

                let children_view = if n.is_display && has_kids {
                    let kid_ids_sv = StoredValue::new(
                        kids.into_iter().map(|c| c.id).collect::<Vec<String>>(),
                    );

                    view! {
                        <For
                            each=move || kid_ids_sv.get_value()
                            key=|id| id.clone()
                            children=move |id| {
                                let nid = note_id_sv.get_value();
                                view! {
                                    <OutlineNode
                                        nav_id=id
                                        depth=depth + 1
                                        navs=navs
                                        note_id=nid
                                        editing_id=editing_id
                                        editing_value=editing_value
                                        target_cursor_col=target_cursor_col
                                        editing_ref=editing_ref
                                        focused_nav_id=focused_nav_id
                                    />
                                }
                            }
                        />
                    }
                    .into_any()
                } else {
                    ().into_view().into_any()
                };

                view! {
                    <div>
                        <div style=move || format!("padding-left: {}px", indent_px)>
                            <div
                                id=move || format!("nav-{}", nav_id_sv.get_value())
                                class=move || {
                                    let id = nav_id_sv.get_value();
                                    let is_editing = editing_id.get().as_deref() == Some(id.as_str());
                                    let is_focused = focused_nav_id.get().as_deref() == Some(id.as_str());

                                    if is_editing {
                                        "outline-row outline-row--editing flex items-center gap-2 py-1"
                                    } else if is_focused {
                                        // Temporary highlight when jumping from backlinks.
                                        "outline-row flex items-center gap-2 py-1 rounded-md bg-primary/10 ring-1 ring-primary/30"
                                    } else {
                                        "outline-row flex items-center gap-2 py-1"
                                    }
                                }
                                draggable="true"
                                on:dragstart=move |ev: web_sys::DragEvent| {
                                    if let Some(dt) = ev.data_transfer() {
                                        let _ = dt.set_data("text/plain", &nav_id_sv.get_value());
                                        dt.set_drop_effect("move");
                                    }
                                }
                                on:dragover=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default();
                                    if let Some(dt) = ev.data_transfer() {
                                        dt.set_drop_effect("move");
                                    }
                                }
                                on:drop=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default();

                                    let dragged_id = ev
                                        .data_transfer()
                                        .and_then(|dt| dt.get_data("text/plain").ok())
                                        .unwrap_or_default();
                                    if dragged_id.trim().is_empty() {
                                        return;
                                    }
                                    if is_tmp_nav_id(&dragged_id) {
                                        return;
                                    }

                                    let target_id = nav_id_sv.get_value();
                                    if dragged_id == target_id {
                                        return;
                                    }

                                    // Decide before/after by cursor position inside target row.
                                    let insert_after = ev
                                        .current_target()
                                        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                                        .map(|el| el.get_bounding_client_rect())
                                        .map(|rect| {
                                            let mid = rect.top() + rect.height() / 2.0;
                                            (ev.client_y() as f64) >= mid
                                        })
                                        .unwrap_or(true);

                                    let note_id_now = note_id_sv.get_value();
                                    let all = navs.get_untracked();
                                    let Some((new_parid, new_order)) =
                                        compute_reorder_target(&all, &dragged_id, &target_id, insert_after)
                                    else {
                                        return;
                                    };

                                    // Update local state.
                                    navs.update(|xs| {
                                        if let Some(x) = xs.iter_mut().find(|x| x.id == dragged_id) {
                                            x.parid = new_parid.clone();
                                            x.same_deep_order = new_order;
                                        }
                                    });

                                    // Persist to backend.
                                    let api_client = app_state.0.api_client.get_untracked();
                                    let req = CreateOrUpdateNavRequest {
                                        note_id: note_id_now,
                                        id: Some(dragged_id),
                                        parid: Some(new_parid),
                                        content: None,
                                        order: Some(new_order),
                                        is_display: None,
                                        is_delete: None,
                                        properties: None,
                                    };
                                    spawn_local(async move {
                                        let _ = api_client.upsert_nav(req).await;
                                    });
                                }
                            >
                            <button
                                class=bullet_class
                                on:click=move |ev| on_toggle_cb.run(ev)
                                disabled=!has_kids
                                title=move || if has_kids {
                                    if n.is_display { "Collapse" } else { "Expand" }
                                } else {
                                    ""
                                }
                            >
                                {bullet}
                            </button>

                            <div class="min-w-0 flex-1 text-sm">
                                {move || {
                                    let id = nav_id_sv.get_value();
                                    let is_editing = editing_id.get().as_deref() == Some(id.as_str());

                                    if !is_editing {
                                        let content_now = n.content.clone();
                                        let content_for_click = content_now.clone();

                                        // Ensure empty nodes still have a clickable area.
                                        let content_display = if content_now.trim().is_empty() {
                                            "\u{00A0}".to_string()
                                        } else {
                                            content_now
                                        };

                                        let id_for_click = nav_id_sv.get_value();

                                        // navigate provided by component scope
                                        let tokens = parse_wiki_tokens(&content_display);

                                        return view! {
                                            <div
                                                class="cursor-text whitespace-pre-wrap min-h-[20px]"
                                                on:mousedown=move |_ev: web_sys::MouseEvent| {
                                                    // Use mousedown (not click) for single-click switching.
                                                    // IMPORTANT: don't rely on `blur` to save. When a focused input is
                                                    // unmounted by state updates, browsers may not fire blur reliably.
                                                    // Save the current editing buffer explicitly before switching.

                                                    if let Some(current_id) = editing_id.get_untracked() {
                                                        let current_content = editing_value.get_untracked();

                                                        // Update local state.
                                                        navs.update(|xs| {
                                                            let _ = apply_nav_content(xs, &current_id, &current_content);
                                                        });

                                                        // Persist to backend.
                                                        let api_client = app_state.0.api_client.get_untracked();
                                                        let note_id_now = note_id_sv.get_value();
                                                        let req = CreateOrUpdateNavRequest {
                                                            note_id: note_id_now,
                                                            id: Some(current_id.clone()),
                                                            parid: None,
                                                            content: Some(current_content),
                                                            order: None,
                                                            is_display: None,
                                                            is_delete: None,
                                                            properties: None,
                                                        };
                                                        spawn_local(async move {
                                                            let _ = api_client.upsert_nav(req).await;
                                                        });
                                                    }

                                                    // Defer the actual switch so the current input can unmount cleanly.
                                                    let id = id_for_click.clone();
                                                    let next_value = content_for_click.clone();
                                                    let editing_id = editing_id;
                                                    let editing_value = editing_value;

                                                    let cb = Closure::<dyn FnMut()>::new(move || {
                                                        editing_id.set(Some(id.clone()));
                                                        editing_value.set(next_value.clone());
                                                    });
                                                    let _ = window()
                                                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                                                            cb.as_ref().unchecked_ref(),
                                                            0,
                                                        );
                                                    cb.forget();
                                                }
                                            >
                                                {{
                                                    let app_state_for_tokens = app_state_sv.get_value();
                                                    let navigate_for_tokens = navigate_sv.get_value();

                                                    tokens
                                                        .into_iter()
                                                        .map(move |t| {
                                                            let app_state = app_state_for_tokens.clone();
                                                            let navigate = navigate_for_tokens.clone();
                                                            match t {
                                                                WikiToken::Text(s) => {
                                                                    view! { <span>{s}</span> }.into_any()
                                                                }
                                                                WikiToken::Link(label) => {
                                                                    let title_raw = label;
                                                                    if title_raw.is_empty() {
                                                                        return view! { <span>"[[]]"</span> }.into_any();
                                                                    }

                                                                    let title_display = title_raw.clone();
                                                                    let title_preview_title = title_raw.clone();

                                                                    let title_for_click = title_raw.clone();
                                                                    let _title_for_title = title_for_click.clone();

                                                                    // Avoid moving `app_state` into one handler and breaking the other.
                                                                    let app_state_hover = app_state.clone();
                                                                    let app_state_click = app_state.clone();

                                                                    // Hover preview: title + first N navs (best-effort).
                                                                    let preview_open: RwSignal<bool> = RwSignal::new(false);
                                                                    let preview_loading: RwSignal<bool> = RwSignal::new(false);
                                                                    let preview_error: RwSignal<Option<String>> = RwSignal::new(None);
                                                                    let preview_lines: RwSignal<Vec<String>> = RwSignal::new(vec![]);
                                                                    let preview_loaded_for: RwSignal<Option<String>> = RwSignal::new(None);

                                                                    let title_for_hover = title_raw.clone();

                                                                    view! {
                                                                        <span class="relative inline-block"
                                                                            on:mouseenter=move |_ev: web_sys::MouseEvent| {
                                                                                preview_open.set(true);

                                                                                // Avoid refetching while hovering same title.
                                                                                if preview_loaded_for.get_untracked().as_deref() == Some(title_for_hover.as_str()) {
                                                                                    return;
                                                                                }
                                                                                preview_loaded_for.set(Some(title_for_hover.clone()));
                                                                                preview_loading.set(true);
                                                                                preview_error.set(None);
                                                                                preview_lines.set(vec![]);

                                                                                let title = title_for_hover.clone();
                                                                                let title_norm = normalize_roam_page_title(&title);

                                                                                let db_id = app_state_hover
                                                                                    .0
                                                                                    .current_database_id
                                                                                    .get_untracked()
                                                                                    .unwrap_or_default();
                                                                                let notes = app_state_hover.0.notes.get_untracked();
                                                                                let api_client = app_state_hover.0.api_client.get_untracked();
                                                                                let app_state_hover2 = app_state_hover.clone();

                                                                                spawn_local(async move {
                                                                                    // 1) Try local notes first.
                                                                                    let mut note_id_opt = notes
                                                                                        .iter()
                                                                                        .find(|n| {
                                                                                            n.database_id == db_id
                                                                                                && normalize_roam_page_title(&n.title) == title_norm
                                                                                        })
                                                                                        .map(|n| n.id.clone());

                                                                                    // 2) If missing, refresh note list (best-effort) and try again.
                                                                                    if note_id_opt.is_none() {
                                                                                        match api_client.get_all_note_list(&db_id).await {
                                                                                            Ok(notes2) => {
                                                                                                // Keep global notes cache in sync so future hovers/clicks work.
                                                                                                app_state_hover2.0.notes.set(notes2.clone());

                                                                                                note_id_opt = notes2
                                                                                                    .iter()
                                                                                                    .find(|n| {
                                                                                                        n.database_id == db_id
                                                                                                            && normalize_roam_page_title(&n.title) == title_norm
                                                                                                    })
                                                                                                    .map(|n| n.id.clone());
                                                                                            }
                                                                                            Err(e) => {
                                                                                                preview_error.set(Some(e));
                                                                                            }
                                                                                        }
                                                                                    }

                                                                                    let Some(note_id) = note_id_opt else {
                                                                                        // Unreferenced (not created yet) is OK.
                                                                                        preview_loading.set(false);
                                                                                        return;
                                                                                    };

                                                                                    match api_client.get_note_navs(&note_id).await {
                                                                                        Ok(navs) => {
                                                                                            // Build a visible preorder list with indentation.
                                                                                            let root = "00000000-0000-0000-0000-000000000000";

                                                                                            let mut by_parent: std::collections::HashMap<String, Vec<Nav>> =
                                                                                                std::collections::HashMap::new();
                                                                                            for n in navs.into_iter() {
                                                                                                if n.is_delete {
                                                                                                    continue;
                                                                                                }
                                                                                                by_parent.entry(n.parid.clone()).or_default().push(n);
                                                                                            }
                                                                                            for (_k, xs) in by_parent.iter_mut() {
                                                                                                xs.sort_by(|a, b| a
                                                                                                    .same_deep_order
                                                                                                    .partial_cmp(&b.same_deep_order)
                                                                                                    .unwrap_or(std::cmp::Ordering::Equal));
                                                                                            }

                                                                                            let mut out: Vec<String> = vec![];
                                                                                            fn walk(
                                                                                                by_parent: &std::collections::HashMap<String, Vec<Nav>>,
                                                                                                parid: &str,
                                                                                                depth: usize,
                                                                                                out: &mut Vec<String>,
                                                                                                limit: usize,
                                                                                            ) {
                                                                                                if out.len() >= limit {
                                                                                                    return;
                                                                                                }
                                                                                                let Some(kids) = by_parent.get(parid) else { return; };
                                                                                                for n in kids.iter() {
                                                                                                    if out.len() >= limit {
                                                                                                        return;
                                                                                                    }
                                                                                                    let indent = "  ".repeat(depth);
                                                                                                    let line = format!("{}{}", indent, n.content);
                                                                                                    out.push(line);
                                                                                                    if n.is_display {
                                                                                                        walk(by_parent, &n.id, depth + 1, out, limit);
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                            walk(&by_parent, root, 0, &mut out, 8);
                                                                                            preview_lines.set(out);
                                                                                        }
                                                                                        Err(e) => {
                                                                                            preview_error.set(Some(e));
                                                                                        }
                                                                                    }

                                                                                    preview_loading.set(false);
                                                                                });
                                                                            }
                                                                            on:mouseleave=move |_ev: web_sys::MouseEvent| {
                                                                                preview_open.set(false);
                                                                            }
                                                                        >
                                                                            <a
                                                                                class="text-primary underline underline-offset-2 hover:text-primary/80"
                                                                                href="#"
                                                                                title=""
                                                                                on:mousedown=move |ev: web_sys::MouseEvent| {
                                                                                    // Only handle left click.
                                                                                    // Right click should open context menu, not navigate.
                                                                                    if ev.button() != 0 {
                                                                                        return;
                                                                                    }

                                                                                    // Prevent switching into edit mode.
                                                                                    ev.prevent_default();
                                                                                    ev.stop_propagation();

                                                                                    let title = title_for_click.clone();
                                                                                    let title_norm = normalize_roam_page_title(&title);

                                                                                    let db_id = app_state_click
                                                                                        .0
                                                                                        .current_database_id
                                                                                        .get_untracked()
                                                                                        .unwrap_or_default();
                                                                                    if db_id.trim().is_empty() {
                                                                                        return;
                                                                                    }

                                                                                    let api_client = app_state_click.0.api_client.get_untracked();
                                                                                    let navigate2 = navigate.clone();
                                                                                    let app_state2 = app_state_click.clone();
                                                                                    spawn_local(async move {
                                                                                        let find_existing_id = |notes: &[Note]| {
                                                                                            notes
                                                                                                .iter()
                                                                                                .find(|n| {
                                                                                                    n.database_id == db_id
                                                                                                        && normalize_roam_page_title(&n.title)
                                                                                                            == title_norm
                                                                                                })
                                                                                                .map(|n| n.id.clone())
                                                                                        };

                                                                                        // 1) Try local notes first.
                                                                                        if let Some(id) = find_existing_id(&app_state2.0.notes.get_untracked()) {
                                                                                            navigate2(
                                                                                                &format!("/db/{}/note/{}", db_id, id),
                                                                                                leptos_router::NavigateOptions::default(),
                                                                                            );
                                                                                            return;
                                                                                        }

                                                                                        // 2) Refresh notes list for this DB, then try again.
                                                                                        if let Ok(notes) = api_client.get_all_note_list(&db_id).await {
                                                                                            app_state2.0.notes.set(notes.clone());
                                                                                            if let Some(id) = find_existing_id(&notes) {
                                                                                                navigate2(
                                                                                                    &format!("/db/{}/note/{}", db_id, id),
                                                                                                    leptos_router::NavigateOptions::default(),
                                                                                                );
                                                                                                return;
                                                                                            }
                                                                                        }

                                                                                        // 3) Still not found: open draft note view (Roam-style, no create on click).
                                                                                        navigate2(
                                                                                            &format!(
                                                                                                "/db/{}/note?title={}",
                                                                                                db_id,
                                                                                                urlencoding::encode(&title)
                                                                                            ),
                                                                                            leptos_router::NavigateOptions::default(),
                                                                                        );
                                                                                    });
                                                                                }
                                                                            >
                                                                                "[["{title_display}"]]"
                                                                            </a>

                                                                            <Show when=move || preview_open.get()>
                                                                                <div class="absolute left-0 top-full mt-1 w-[28rem] max-w-[90vw] rounded-md border border-border bg-background text-foreground p-3 text-xs shadow-lg">
                                                                                    <div class="font-medium truncate">{title_preview_title.clone()}</div>
                                                                                    <Show when=move || preview_loading.get() fallback=|| ().into_view()>
                                                                                        <div class="mt-2 text-muted-foreground">"Loading…"</div>
                                                                                    </Show>
                                                                                    <Show when=move || preview_error.get().is_some() fallback=|| ().into_view()>
                                                                                        <div class="mt-2 text-destructive">{move || preview_error.get().unwrap_or_default()}</div>
                                                                                    </Show>
                                                                                    <Show
                                                                                        when=move || !preview_loading.get() && preview_error.get().is_none()
                                                                                        fallback=|| ().into_view()
                                                                                    >
                                                                                        {move || {
                                                                                            let lines = preview_lines.get();
                                                                                            if lines.is_empty() {
                                                                                                return view! { <div class="mt-2 text-muted-foreground">"No content (page may not exist yet)."</div> }.into_any();
                                                                                            }
                                                                                            view! {
                                                                                                <div class="mt-2 space-y-1">
                                                                                                    {lines
                                                                                                        .into_iter()
                                                                                                        .map(|l| view! { <div class="whitespace-pre-wrap break-words">{l}</div> })
                                                                                                        .collect_view()}
                                                                                                </div>
                                                                                            }
                                                                                            .into_any()
                                                                                        }}
                                                                                    </Show>
                                                                                </div>
                                                                            </Show>
                                                                        </span>
                                                                    }
                                                                    .into_any()
                                                                }
                                                            }
                                                        })
                                                        .collect_view()
                                                }}
                                            </div>
                                        }
                                        .into_any();
                                    }

                                    view! {
                                        <div class="relative">
                                        <input
                                            node_ref=editing_ref
                                            // Store stable ids on the DOM node so blur handlers can read them even if
                                            // reactive values are disposed during navigation/unmount.
                                            attr:data-nav-id=nav_id_sv.get_value()
                                            attr:data-note-id=note_id_sv.get_value()
                                            class="h-7 w-full min-w-0 flex-1 rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50"
                                            value=move || editing_value.get()
                                            on:input=move |ev: web_sys::Event| {
                                                let v = event_target_value(&ev);
                                                editing_value.set(v.clone());

                                                // Autocomplete: detect an unclosed `[[...` immediately before the caret.
                                                let caret_utf16 = ev
                                                    .target()
                                                    .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                    .and_then(|el| el.selection_start().ok().flatten())
                                                    .unwrap_or(v.encode_utf16().count() as u32);

                                                let caret_byte = utf16_to_byte_idx(&v, caret_utf16);
                                                let prefix = &v[..caret_byte.min(v.len())];

                                                let ac = ac_sv.get_value();
                                                let app_state = app_state_sv.get_value();

                                                let Some(start_byte) = prefix.rfind("[[") else {
                                                    ac.ac_open.set(false);
                                                    ac.ac_start_utf16.set(None);
                                                    return;
                                                };

                                                // If the user already closed the link before the caret, don't autocomplete.
                                                if prefix[start_byte..].contains("]]") {
                                                    ac.ac_open.set(false);
                                                    ac.ac_start_utf16.set(None);
                                                    return;
                                                }

                                                let q = prefix[start_byte + 2..].to_string();
                                                ac.ac_query.set(q.clone());
                                                ac.ac_start_utf16
                                                    .set(Some(byte_idx_to_utf16(&v, start_byte)));

                                                // Load titles lazily (notes + wiki links across DB).
                                                ensure_titles_loaded(&app_state, &ac);

                                                // If titles are still loading, keep the menu open and let the
                                                // recompute Effect populate items once loading completes.
                                                if ac.titles_loading.get_untracked() {
                                                    ac.ac_open.set(true);
                                                    ac.ac_index.set(0);
                                                    ac.ac_items.set(vec![]);
                                                    return;
                                                }

                                                let titles = ac.titles_cache.get_untracked();
                                                let items = build_ac_items(&titles, &q);

                                                if items.is_empty() {
                                                    ac.ac_open.set(false);
                                                    ac.ac_index.set(0);
                                                    return;
                                                }

                                                ac.ac_items.set(items);
                                                ac.ac_index.set(0);
                                                ac.ac_open.set(true);
                                            }
                                            on:blur=move |ev| {
                                                let ac = ac_sv.get_value();

                                                // Close autocomplete if open.
                                                ac.ac_open.set(false);
                                                ac.ac_start_utf16.set(None);

                                                // IMPORTANT: read the value from the input element.
                                                let new_content = event_target_value(&ev);

                                                // Navigation can unmount this component before blur runs.
                                                // Reading StoredValue/signal here can panic if it's already disposed.
                                                // Instead, read ids from the DOM attributes.
                                                let (nav_id_now, note_id_now) = ev
                                                    .target()
                                                    .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                    .map(|el| {
                                                        (
                                                            el.get_attribute("data-nav-id").unwrap_or_default(),
                                                            el.get_attribute("data-note-id").unwrap_or_default(),
                                                        )
                                                    })
                                                    .unwrap_or_default();

                                                // If the input is already being torn down (e.g. Enter triggers a state
                                                // change and the blur fires late), we may not be able to recover ids.
                                                // In that case, don't send an invalid request.
                                                if nav_id_now.trim().is_empty()
                                                    || note_id_now.trim().is_empty()
                                                    || is_tmp_nav_id(&nav_id_now)
                                                {
                                                    return;
                                                }

                                                // Clear editing if we are still editing this node.
                                                if editing_id.get_untracked().as_deref() == Some(nav_id_now.as_str()) {
                                                    editing_id.set(None);
                                                }

                                                navs.update(|xs| {
                                                    let _ = apply_nav_content(xs, &nav_id_now, &new_content);
                                                });

                                                let api_client = app_state.0.api_client.get_untracked();
                                                let req = CreateOrUpdateNavRequest {
                                                    note_id: note_id_now,
                                                    id: Some(nav_id_now.clone()),
                                                    parid: None,
                                                    content: Some(new_content),
                                                    order: None,
                                                    is_display: None,
                                                    is_delete: None,
                                                    properties: None,
                                                };
                                                spawn_local(async move {
                                                    let _ = api_client.upsert_nav(req).await;
                                                });
                                            }
                                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                let key = ev.key();

                                                // Helpers for reading the current input element.
                                                let input = || {
                                                    ev.target()
                                                        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                };

                                                let ac = ac_sv.get_value();

                                                // Autocomplete menu key handling.
                                                if ac.ac_open.get_untracked() {
                                                    match key.as_str() {
                                                        "ArrowDown" => {
                                                            ev.prevent_default();
                                                            let len = ac.ac_items.get_untracked().len();
                                                            if len > 0 {
                                                                ac.ac_index.update(|i| *i = (*i + 1).min(len - 1));
                                                            }
                                                            return;
                                                        }
                                                        "ArrowUp" => {
                                                            ev.prevent_default();
                                                            ac.ac_index.update(|i| *i = i.saturating_sub(1));
                                                            return;
                                                        }
                                                        "Escape" => {
                                                            ev.prevent_default();
                                                            ac.ac_open.set(false);
                                                            return;
                                                        }
                                                        "Enter" | "Tab" => {
                                                            ev.prevent_default();
                                                            let items = ac.ac_items.get_untracked();
                                                            let idx = ac.ac_index.get_untracked();
                                                            if let Some(item) = items.get(idx) {
                                                                let chosen = item.title.clone();

                                                                if let Some(input_el) = input() {
                                                                    let v = input_el.value();
                                                                    let caret_utf16 = input_el
                                                                        .selection_start()
                                                                        .ok()
                                                                        .flatten()
                                                                        .unwrap_or(v.encode_utf16().count() as u32);

                                                                    let caret_byte = utf16_to_byte_idx(&v, caret_utf16);
                                                                    let start_utf16 = ac.ac_start_utf16.get_untracked().unwrap_or(0);
                                                                    let start_byte = utf16_to_byte_idx(&v, start_utf16);

                                                                    let mut next = String::new();
                                                                    next.push_str(&v[..start_byte.min(v.len())]);
                                                                    next.push_str("[[");
                                                                    next.push_str(&chosen);
                                                                    next.push_str("]]" );
                                                                    next.push_str(&v[caret_byte.min(v.len())..]);

                                                                    input_el.set_value(&next);
                                                                    editing_value.set(next.clone());

                                                                    let caret_after = start_utf16
                                                                        + 2
                                                                        + (chosen.encode_utf16().count() as u32)
                                                                        + 2;
                                                                    let _ = input_el.set_selection_range(caret_after, caret_after);
                                                                }

                                                                ac.ac_open.set(false);
                                                                ac.ac_start_utf16.set(None);
                                                            }
                                                            return;
                                                        }
                                                        _ => {}
                                                    }
                                                }

                                                // Helpers for Roam-style navigation

                                                let save_current = |nav_id_now: &str, note_id_now: &str| {
                                                    let current_content = editing_value.get_untracked();
                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                        }
                                                    });

                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    let save_req = CreateOrUpdateNavRequest {
                                                        note_id: note_id_now.to_string(),
                                                        id: Some(nav_id_now.to_string()),
                                                        parid: None,
                                                        content: Some(current_content),
                                                        order: None,
                                                        is_display: None,
                                                        is_delete: None,
                                                        properties: None,
                                                    };

                                                    spawn_local(async move {
                                                        let _ = api_client.upsert_nav(save_req).await;
                                                    });
                                                };

                                                fn visible_preorder(all: &[Nav]) -> Vec<String> {
                                                    let root = "00000000-0000-0000-0000-000000000000";

                                                    fn children_sorted(all: &[Nav], parid: &str) -> Vec<Nav> {
                                                        let mut out = all
                                                            .iter()
                                                            .filter(|n| n.parid == parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        out.sort_by(|a, b| {
                                                            a.same_deep_order
                                                                .partial_cmp(&b.same_deep_order)
                                                                .unwrap_or(std::cmp::Ordering::Equal)
                                                        });
                                                        out
                                                    }

                                                    fn collect(all: &[Nav], parid: &str, out: &mut Vec<String>) {
                                                        for n in children_sorted(all, parid) {
                                                            out.push(n.id.clone());
                                                            if n.is_display {
                                                                collect(all, &n.id, out);
                                                            }
                                                        }
                                                    }

                                                    let mut out: Vec<String> = vec![];
                                                    collect(all, root, &mut out);
                                                    out
                                                }

                                                // Alt+Up/Down: move current node among siblings (order only)
                                                if ev.alt_key() && (key == "ArrowUp" || key == "ArrowDown") {
                                                    ev.prevent_default();

                                                    let cursor_col = input()
                                                        .as_ref()
                                                        .and_then(|i| i.selection_start().ok().flatten())
                                                        .unwrap_or(0);
                                                    target_cursor_col.set(Some(cursor_col));

                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();
                                                    let current_content = editing_value.get_untracked();

                                                    let all = navs.get_untracked();
                                                    let Some(me) = all.iter().find(|n| n.id == nav_id_now) else {
                                                        return;
                                                    };

                                                    // Siblings sorted by order.
                                                    let parid = me.parid.clone();
                                                    let mut sibs = all
                                                        .iter()
                                                        .filter(|n| n.parid == parid)
                                                        .cloned()
                                                        .collect::<Vec<_>>();
                                                    sibs.sort_by(|a, b| {
                                                        a.same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal)
                                                    });

                                                    let idx = sibs.iter().position(|n| n.id == nav_id_now);
                                                    let Some(idx) = idx else { return; };

                                                    // Compute new order by placing between adjacent siblings.
                                                    let new_order = if key == "ArrowUp" {
                                                        if idx == 0 {
                                                            // Already first.
                                                            return;
                                                        }
                                                        let prev = &sibs[idx - 1];
                                                        let prevprev_order = if idx >= 2 {
                                                            sibs[idx - 2].same_deep_order
                                                        } else {
                                                            prev.same_deep_order - 1.0
                                                        };
                                                        (prevprev_order + prev.same_deep_order) / 2.0
                                                    } else {
                                                        if idx + 1 >= sibs.len() {
                                                            // Already last.
                                                            return;
                                                        }
                                                        let next = &sibs[idx + 1];
                                                        let nextnext_order = if idx + 2 < sibs.len() {
                                                            sibs[idx + 2].same_deep_order
                                                        } else {
                                                            next.same_deep_order + 1.0
                                                        };
                                                        (next.same_deep_order + nextnext_order) / 2.0
                                                    };

                                                    // Update local state.
                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                            x.same_deep_order = new_order;
                                                        }

                                                        // Keep navs unsorted: rendering and navigation sort per-parent using
                                                        // `same_deep_order`, so globally sorting the whole list is unnecessary
                                                        // work (and gets slower as the outline grows).
                                                    });

                                                    // Persist to backend.
                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    let req = CreateOrUpdateNavRequest {
                                                        note_id: note_id_now,
                                                        id: Some(nav_id_now.clone()),
                                                        parid: None,
                                                        content: Some(current_content),
                                                        order: Some(new_order),
                                                        is_display: None,
                                                        is_delete: None,
                                                        properties: None,
                                                    };
                                                    spawn_local(async move {
                                                        let _ = api_client.upsert_nav(req).await;
                                                    });

                                                    // Keep editing current node.
                                                    editing_id.set(Some(nav_id_now));
                                                    return;
                                                }

                                                // Arrow Up/Down: move between visible nodes
                                                if key == "ArrowUp" || key == "ArrowDown" {
                                                    ev.prevent_default();

                                                    let cursor_col = input()
                                                        .as_ref()
                                                        .and_then(|i| i.selection_start().ok().flatten())
                                                        .unwrap_or(0);
                                                    target_cursor_col.set(Some(cursor_col));

                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();
                                                    save_current(&nav_id_now, &note_id_now);

                                                    let all = navs.get_untracked();
                                                    let visible = visible_preorder(&all);

                                                    let idx = visible.iter().position(|id| id == &nav_id_now);
                                                    let Some(idx) = idx else { return; };

                                                    let next_id = if key == "ArrowUp" {
                                                        if idx == 0 { None } else { Some(visible[idx - 1].clone()) }
                                                    } else {
                                                        if idx + 1 >= visible.len() { None } else { Some(visible[idx + 1].clone()) }
                                                    };

                                                    if let Some(next_id) = next_id {
                                                        if let Some(next_nav) = all.iter().find(|n| n.id == next_id) {
                                                            editing_id.set(Some(next_id));
                                                            editing_value.set(next_nav.content.clone());
                                                        }
                                                    }

                                                    return;
                                                }

                                                // Arrow Left/Right: jump to prev/next visible node at boundaries
                                                if key == "ArrowLeft" || key == "ArrowRight" {
                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    let (cursor_start, cursor_end, len) = if let Some(i) = input() {
                                                        let start = i.selection_start().ok().flatten().unwrap_or(0);
                                                        let end = i.selection_end().ok().flatten().unwrap_or(start);
                                                        // IMPORTANT: selectionStart/End use UTF-16 code units, not Rust UTF-8 bytes.
                                                        let len = i.value().encode_utf16().count() as u32;
                                                        (start, end, len)
                                                    } else {
                                                        (0, 0, 0)
                                                    };

                                                    // Only trigger when selection is collapsed.
                                                    if cursor_start != cursor_end {
                                                        return;
                                                    }

                                                    if key == "ArrowLeft" && cursor_start == 0 {
                                                        ev.prevent_default();
                                                        target_cursor_col.set(None);
                                                        save_current(&nav_id_now, &note_id_now);

                                                        let all = navs.get_untracked();
                                                        let Some(me) = all.iter().find(|n| n.id == nav_id_now) else {
                                                            return;
                                                        };

                                                        let root = "00000000-0000-0000-0000-000000000000";

                                                        // Prefer previous sibling when it exists.
                                                        // If there is no previous sibling (i.e. first child), go to parent.
                                                        let parid = me.parid.clone();
                                                        let mut sibs = all
                                                            .iter()
                                                            .filter(|n| n.parid == parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        sibs.sort_by(|a, b| a
                                                            .same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        let prev = sibs
                                                            .iter()
                                                            .rev()
                                                            .find(|s| s.same_deep_order < me.same_deep_order)
                                                            .cloned();

                                                        if prev.is_none() {
                                                            if me.parid != root {
                                                                if let Some(parent) = all.iter().find(|n| n.id == me.parid) {
                                                                    editing_id.set(Some(parent.id.clone()));
                                                                    editing_value.set(parent.content.clone());
                                                                    target_cursor_col.set(Some(parent.content.encode_utf16().count() as u32));
                                                                }
                                                            }
                                                            return;
                                                        }

                                                        let prev = prev.unwrap();

                                                        // Descend to last visible node in prev's subtree.
                                                        fn last_visible_descendant(all: &[Nav], start: &Nav) -> Nav {
                                                            if !start.is_display {
                                                                return start.clone();
                                                            }
                                                            let mut children = all
                                                                .iter()
                                                                .filter(|n| n.parid == start.id)
                                                                .cloned()
                                                                .collect::<Vec<_>>();
                                                            children.sort_by(|a, b| a
                                                                .same_deep_order
                                                                .partial_cmp(&b.same_deep_order)
                                                                .unwrap_or(std::cmp::Ordering::Equal));
                                                            if let Some(last) = children.last() {
                                                                return last_visible_descendant(all, last);
                                                            }
                                                            start.clone()
                                                        }

                                                        let target = last_visible_descendant(&all, &prev);
                                                        editing_id.set(Some(target.id.clone()));
                                                        editing_value.set(target.content.clone());
                                                        target_cursor_col.set(Some(target.content.encode_utf16().count() as u32));
                                                        return;
                                                    }

                                                    if key == "ArrowRight" && cursor_start == len {
                                                        ev.prevent_default();
                                                        target_cursor_col.set(None);
                                                        save_current(&nav_id_now, &note_id_now);

                                                        let all = navs.get_untracked();

                                                        // Roam-ish behavior: if node has children and is collapsed, expand.
                                                        // If expanded, move into first child.
                                                        let mut children = all
                                                            .iter()
                                                            .filter(|n| n.parid == nav_id_now)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        children.sort_by(|a, b| a
                                                            .same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        if let Some(first_child) = children.first().cloned() {
                                                            let is_display = all
                                                                .iter()
                                                                .find(|n| n.id == nav_id_now)
                                                                .map(|n| n.is_display)
                                                                .unwrap_or(true);

                                                            if !is_display {
                                                                // Expand current node AND descend into first child.
                                                                // This matches Roam's feel: Right at end opens and goes deeper.
                                                                navs.update(|xs| {
                                                                    if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                                        x.is_display = true;
                                                                    }
                                                                });

                                                                let api_client = app_state.0.api_client.get_untracked();
                                                                let req = CreateOrUpdateNavRequest {
                                                                    note_id: note_id_now.clone(),
                                                                    id: Some(nav_id_now.clone()),
                                                                    parid: None,
                                                                    content: None,
                                                                    order: None,
                                                                    is_display: Some(true),
                                                                    is_delete: None,
                                                                    properties: None,
                                                                };
                                                                spawn_local(async move {
                                                                    let _ = api_client.upsert_nav(req).await;
                                                                });

                                                                editing_id.set(Some(first_child.id.clone()));
                                                                editing_value.set(first_child.content.clone());
                                                                target_cursor_col.set(Some(0));
                                                                return;
                                                            }

                                                            // Move into first child.
                                                            editing_id.set(Some(first_child.id.clone()));
                                                            editing_value.set(first_child.content.clone());
                                                            target_cursor_col.set(Some(0));
                                                            return;
                                                        }

                                                        // Strict Roam behavior: if there are no children, ArrowRight does not move to a sibling.
                                                        return;
                                                    }
                                                }

                                                // Tab / Shift+Tab: indent / outdent
                                                if key == "Tab" {
                                                    ev.prevent_default();

                                                    let shift = ev.shift_key();
                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    let all = navs.get_untracked();
                                                    let Some(me) = all.iter().find(|x| x.id == nav_id_now) else {
                                                        return;
                                                    };

                                                    // Save current edit buffer into local state first.
                                                    let current_content = editing_value.get_untracked();
                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                        }
                                                    });

                                                    let api_client = app_state.0.api_client.get_untracked();

                                                    if !shift {
                                                        // Indent: become child of previous sibling.
                                                        let parid = me.parid.clone();
                                                        let mut sibs = all
                                                            .iter()
                                                            .filter(|x| x.parid == parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        sibs.sort_by(|a, b| a.same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        let prev = sibs
                                                            .iter()
                                                            .rev()
                                                            .find(|s| s.same_deep_order < me.same_deep_order)
                                                            .cloned();

                                                        let Some(prev) = prev else {
                                                            return;
                                                        };

                                                        let new_parid = prev.id.clone();

                                                        // Append to end of new parent's children.
                                                        let last_child_order = all
                                                            .iter()
                                                            .filter(|x| x.parid == new_parid)
                                                            .map(|x| x.same_deep_order)
                                                            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                                                        let new_order = last_child_order.unwrap_or(0.0) + 1.0;

                                                        navs.update(|xs| {
                                                            if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                                x.parid = new_parid.clone();
                                                                x.same_deep_order = new_order;
                                                            }
                                                            if let Some(p) = xs.iter_mut().find(|x| x.id == new_parid) {
                                                                p.is_display = true;
                                                            }
                                                        });

                                                        let req = CreateOrUpdateNavRequest {
                                                            note_id: note_id_now,
                                                            id: Some(nav_id_now.clone()),
                                                            parid: Some(new_parid),
                                                            content: Some(current_content),
                                                            order: Some(new_order),
                                                            is_display: None,
                                                            is_delete: None,
                                                            properties: None,
                                                        };

                                                        spawn_local(async move {
                                                            let _ = api_client.upsert_nav(req).await;
                                                        });
                                                    } else {
                                                        // Outdent: become sibling of parent.
                                                        let parent_id = me.parid.clone();
                                                        let root = "00000000-0000-0000-0000-000000000000";
                                                        if parent_id == root {
                                                            return;
                                                        }

                                                        let Some(parent) = all.iter().find(|x| x.id == parent_id) else {
                                                            return;
                                                        };

                                                        let new_parid = parent.parid.clone();

                                                        // Put right after parent (midpoint between parent and parent's next sibling).
                                                        let mut parent_sibs = all
                                                            .iter()
                                                            .filter(|x| x.parid == new_parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        parent_sibs.sort_by(|a, b| a.same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        let next_order = parent_sibs
                                                            .iter()
                                                            .find(|s| s.same_deep_order > parent.same_deep_order)
                                                            .map(|s| s.same_deep_order);

                                                        let new_order = if let Some(no) = next_order {
                                                            (parent.same_deep_order + no) / 2.0
                                                        } else {
                                                            parent.same_deep_order + 1.0
                                                        };

                                                        navs.update(|xs| {
                                                            if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                                x.parid = new_parid.clone();
                                                                x.same_deep_order = new_order;
                                                            }
                                                        });

                                                        let req = CreateOrUpdateNavRequest {
                                                            note_id: note_id_now,
                                                            id: Some(nav_id_now.clone()),
                                                            parid: Some(new_parid),
                                                            content: Some(current_content),
                                                            order: Some(new_order),
                                                            is_display: None,
                                                            is_delete: None,
                                                            properties: None,
                                                        };

                                                        spawn_local(async move {
                                                            let _ = api_client.upsert_nav(req).await;
                                                        });
                                                    }

                                                    // Keep editing current node.
                                                    editing_id.set(Some(nav_id_now));
                                                    return;
                                                }

                                                // Backspace/Delete on empty: soft-delete node (and its subtree)
                                                if (key == "Backspace" || key == "Delete")
                                                    && editing_value.get_untracked().trim().is_empty()
                                                {
                                                    ev.prevent_default();

                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    let all = navs.get_untracked();

                                                    // Visible order for choosing next focus.
                                                    let visible = visible_preorder(&all);
                                                    let idx = visible.iter().position(|id| id == &nav_id_now);

                                                    // Collect subtree ids (including self).
                                                    fn collect_subtree(all: &[Nav], root_id: &str, out: &mut Vec<String>) {
                                                        out.push(root_id.to_string());
                                                        for c in all.iter().filter(|n| n.parid == root_id) {
                                                            collect_subtree(all, &c.id, out);
                                                        }
                                                    }

                                                    let mut subtree: Vec<String> = vec![];
                                                    collect_subtree(&all, &nav_id_now, &mut subtree);

                                                    // Update local state: remove subtree nodes.
                                                    navs.update(|xs| xs.retain(|n| !subtree.iter().any(|id| id == &n.id)));

                                                    // Pick next focus: previous visible if possible, else next.
                                                    let next_focus = idx
                                                        .and_then(|i| if i > 0 { Some(visible[i - 1].clone()) } else { None })
                                                        .or_else(|| idx.and_then(|i| visible.get(i + 1).cloned()));

                                                    editing_id.set(next_focus.clone());
                                                    if let Some(fid) = next_focus {
                                                        if let Some(n) = all.iter().find(|n| n.id == fid) {
                                                            editing_value.set(n.content.clone());
                                                            target_cursor_col.set(Some(n.content.encode_utf16().count() as u32));
                                                        }
                                                    } else {
                                                        editing_id.set(None);
                                                    }

                                                    // Persist soft delete to backend.
                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    spawn_local(async move {
                                                        for id in subtree {
                                                            let req = CreateOrUpdateNavRequest {
                                                                note_id: note_id_now.clone(),
                                                                id: Some(id),
                                                                parid: None,
                                                                content: None,
                                                                order: None,
                                                                is_display: None,
                                                                is_delete: Some(true),
                                                                properties: None,
                                                            };
                                                            let _ = api_client.upsert_nav(req).await;
                                                        }
                                                    });

                                                    return;
                                                }

                                                // Enter: save + create next sibling
                                                if key == "Enter" {
                                                    ev.prevent_default();

                                                    let current_content = editing_value.get_untracked();
                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                        }
                                                    });

                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    let save_req = CreateOrUpdateNavRequest {
                                                        note_id: note_id_now.clone(),
                                                        id: Some(nav_id_now.clone()),
                                                        parid: None,
                                                        content: Some(current_content.clone()),
                                                        order: None,
                                                        is_display: None,
                                                        is_delete: None,
                                                        properties: None,
                                                    };

                                                    // Create sibling
                                                    let all = navs.get_untracked();
                                                    let Some(me) = all.iter().find(|x| x.id == nav_id_now) else {
                                                        return;
                                                    };

                                                    let parid = me.parid.clone();
                                                    let mut sibs = all
                                                        .iter()
                                                        .filter(|x| x.parid == parid)
                                                        .cloned()
                                                        .collect::<Vec<_>>();
                                                    sibs.sort_by(|a, b| a.same_deep_order
                                                        .partial_cmp(&b.same_deep_order)
                                                        .unwrap_or(std::cmp::Ordering::Equal));

                                                    let next_order = sibs
                                                        .iter()
                                                        .find(|s| s.same_deep_order > me.same_deep_order)
                                                        .map(|s| s.same_deep_order);

                                                    let new_order = if let Some(no) = next_order {
                                                        (me.same_deep_order + no) / 2.0
                                                    } else {
                                                        me.same_deep_order + 1.0
                                                    };

                                                    // Optimistic UI: insert a temporary node locally and start editing it
                                                    // immediately. Replace its id once the backend returns the real id.

                                                    let tmp_id = make_tmp_nav_id(
                                                        js_sys::Date::now() as u64,
                                                        (js_sys::Math::random() * 1e9) as u64,
                                                    );

                                                    navs.update(|xs| {
                                                        xs.push(Nav {
                                                            id: tmp_id.clone(),
                                                            note_id: note_id_now.clone(),
                                                            parid: parid.clone(),
                                                            same_deep_order: new_order,
                                                            content: String::new(),
                                                            is_display: true,
                                                            is_delete: false,
                                                        });
                                                    });

                                                    editing_id.set(Some(tmp_id.clone()));
                                                    editing_value.set(String::new());
                                                    target_cursor_col.set(Some(0));

                                                    spawn_local(async move {
                                                        let _ = api_client.upsert_nav(save_req).await;

                                                        let create_req = CreateOrUpdateNavRequest {
                                                            note_id: note_id_now.clone(),
                                                            id: None,
                                                            parid: Some(parid.clone()),
                                                            content: Some("".to_string()),
                                                            order: Some(new_order),
                                                            is_display: Some(true),
                                                            is_delete: Some(false),
                                                            properties: None,
                                                        };

                                                        if let Ok(resp) = api_client.upsert_nav(create_req).await {
                                                            let new_id = resp
                                                                .get("id")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("")
                                                                .to_string();

                                                            if !new_id.trim().is_empty() {
                                                                // Swap tmp id -> real id.
                                                                let content_now = get_nav_content(
                                                                    &navs.get_untracked(),
                                                                    &tmp_id,
                                                                )
                                                                .unwrap_or_default();

                                                                navs.update(|xs| {
                                                                    let _ = swap_tmp_nav_id(xs, &tmp_id, &new_id);
                                                                });

                                                                // If still editing the tmp node, switch to the real id.
                                                                if editing_id.get_untracked().as_deref() == Some(tmp_id.as_str()) {
                                                                    editing_id.set(Some(new_id.clone()));
                                                                }

                                                                // Persist current content (if user typed before backend returned).
                                                                if let Some(save_req) = backfill_content_request(
                                                                    &note_id_now,
                                                                    &new_id,
                                                                    &content_now,
                                                                ) {
                                                                    let _ = api_client.upsert_nav(save_req).await;
                                                                }
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                        />

                                        <Show when=move || ac_sv.get_value().ac_open.get()>
                                            <div class="absolute z-50 mt-1 w-[28rem] max-w-full rounded-md border border-border bg-background text-foreground p-1 text-sm shadow-lg">
                                                {move || {
                                                    let ac = ac_sv.get_value();
                                                    let items = ac.ac_items.get();
                                                    let idx = ac.ac_index.get();
                                                    if items.is_empty() {
                                                        if ac.titles_loading.get() {
                                                            return view! {
                                                                <div class="px-2 py-1 text-muted-foreground">"Loading…"</div>
                                                            }
                                                            .into_any();
                                                        }
                                                        return ().into_any();
                                                    }

                                                    view! {
                                                        <div class="max-h-64 overflow-auto">
                                                            {items
                                                                .into_iter()
                                                                .enumerate()
                                                                .map(|(i, it)| {
                                                                    let selected = move || i == idx;
                                                                    let title = it.title.clone();
                                                                    let is_new = it.is_new;

                                                                    view! {
                                                                        <div
                                                                            class=move || {
                                                                                if selected() {
                                                                                    "flex cursor-pointer items-center justify-between rounded px-2 py-1 bg-accent text-accent-foreground"
                                                                                } else {
                                                                                    "flex cursor-pointer items-center justify-between rounded px-2 py-1 hover:bg-accent/60"
                                                                                }
                                                                            }
                                                                            on:mousedown=move |ev: web_sys::MouseEvent| {
                                                                                // Prevent input blur.
                                                                                ev.prevent_default();

                                                                                if let Some(input_el) = editing_ref.get() {
                                                                                    let v = input_el.value();
                                                                                    let caret_utf16 = input_el
                                                                                        .selection_start()
                                                                                        .ok()
                                                                                        .flatten()
                                                                                        .unwrap_or(v.encode_utf16().count() as u32);
                                                                                    let caret_byte = utf16_to_byte_idx(&v, caret_utf16);
                                                                                    let start_utf16 = ac.ac_start_utf16.get_untracked().unwrap_or(0);
                                                                                    let start_byte = utf16_to_byte_idx(&v, start_utf16);

                                                                                    let mut next = String::new();
                                                                                    next.push_str(&v[..start_byte.min(v.len())]);
                                                                                    next.push_str("[[");
                                                                                    next.push_str(&title);
                                                                                    next.push_str("]]" );
                                                                                    next.push_str(&v[caret_byte.min(v.len())..]);

                                                                                    input_el.set_value(&next);
                                                                                    editing_value.set(next.clone());

                                                                                    let caret_after = start_utf16
                                                                                        + 2
                                                                                        + (title.encode_utf16().count() as u32)
                                                                                        + 2;
                                                                                    let _ = input_el.set_selection_range(caret_after, caret_after);
                                                                                }

                                                                                ac.ac_open.set(false);
                                                                                ac.ac_start_utf16.set(None);
                                                                            }
                                                                            on:mousemove=move |_ev| {
                                                                                ac.ac_index.set(i);
                                                                            }
                                                                        >
                                                                            <div class="truncate">{title.clone()}</div>
                                                                            <Show when=move || is_new fallback=|| ().into_view()>
                                                                                <div class="ml-2 shrink-0 text-xs text-muted-foreground">"Create"</div>
                                                                            </Show>
                                                                        </div>
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </div>
                                                    }
                                                    .into_any()
                                                }}
                                            </div>
                                        </Show>
                                    </div>
                                    }
                                    .into_any()
                                }}
                            </div>
                        </div>
                        </div>

                        {children_view}
                    </div>
                }
                .into_any()
            }}
        </div>
    }
}

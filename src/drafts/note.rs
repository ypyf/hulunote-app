use crate::storage::{load_json_from_storage, save_json_to_storage};
use crate::util::now_ms;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct FieldDraft {
    pub value: String,
    pub updated_ms: i64,
    pub synced_ms: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct NoteDraft {
    pub db_id: String,
    pub note_id: String,
    pub updated_ms: i64,

    pub title: Option<FieldDraft>,

    /// nav_id -> draft
    #[serde(default)]
    pub navs: BTreeMap<String, FieldDraft>,
}

fn key(db_id: &str, note_id: &str) -> String {
    format!("hulunote_draft_note::{db_id}::{note_id}")
}

fn load_note_draft(db_id: &str, note_id: &str) -> NoteDraft {
    if db_id.trim().is_empty() || note_id.trim().is_empty() {
        return NoteDraft::default();
    }

    load_json_from_storage::<NoteDraft>(&key(db_id, note_id)).unwrap_or_else(|| NoteDraft {
        db_id: db_id.to_string(),
        note_id: note_id.to_string(),
        ..Default::default()
    })
}

fn save_note_draft(d: &NoteDraft) {
    if d.db_id.trim().is_empty() || d.note_id.trim().is_empty() {
        return;
    }
    save_json_to_storage(&key(&d.db_id, &d.note_id), d);
}

pub(crate) fn touch_title(db_id: &str, note_id: &str, title: &str) {
    if db_id.trim().is_empty() || note_id.trim().is_empty() {
        return;
    }

    let mut d = load_note_draft(db_id, note_id);
    let now = now_ms();

    let mut f = d.title.unwrap_or_default();
    f.value = title.to_string();
    f.updated_ms = now;
    // Do not change synced_ms here.

    d.title = Some(f);
    d.updated_ms = now;

    save_note_draft(&d);
}

pub(crate) fn touch_nav(db_id: &str, note_id: &str, nav_id: &str, content: &str) {
    if db_id.trim().is_empty() || note_id.trim().is_empty() || nav_id.trim().is_empty() {
        return;
    }

    let mut d = load_note_draft(db_id, note_id);
    let now = now_ms();

    let f = d
        .navs
        .entry(nav_id.to_string())
        .or_insert_with(FieldDraft::default);
    f.value = content.to_string();
    f.updated_ms = now;

    d.updated_ms = now;

    save_note_draft(&d);
}

pub(crate) fn mark_title_synced(db_id: &str, note_id: &str, synced_ms: i64) {
    if db_id.trim().is_empty() || note_id.trim().is_empty() {
        return;
    }

    let mut d = load_note_draft(db_id, note_id);
    let mut f = d.title.unwrap_or_default();
    f.synced_ms = f.synced_ms.max(synced_ms);
    d.title = Some(f);
    d.updated_ms = now_ms();
    save_note_draft(&d);
}

pub(crate) fn mark_nav_synced(db_id: &str, note_id: &str, nav_id: &str, synced_ms: i64) {
    if db_id.trim().is_empty() || note_id.trim().is_empty() || nav_id.trim().is_empty() {
        return;
    }

    let mut d = load_note_draft(db_id, note_id);
    let f = d
        .navs
        .entry(nav_id.to_string())
        .or_insert_with(FieldDraft::default);
    f.synced_ms = f.synced_ms.max(synced_ms);
    d.updated_ms = now_ms();
    save_note_draft(&d);
}

pub(crate) fn get_title_override(db_id: &str, note_id: &str, server_title: &str) -> String {
    if db_id.trim().is_empty() || note_id.trim().is_empty() {
        return server_title.to_string();
    }

    let d = load_note_draft(db_id, note_id);
    let Some(f) = d.title else {
        return server_title.to_string();
    };

    if f.updated_ms > f.synced_ms {
        f.value
    } else {
        server_title.to_string()
    }
}

pub(crate) fn get_nav_override(
    db_id: &str,
    note_id: &str,
    nav_id: &str,
    server_content: &str,
) -> String {
    if db_id.trim().is_empty() || note_id.trim().is_empty() || nav_id.trim().is_empty() {
        return server_content.to_string();
    }

    let d = load_note_draft(db_id, note_id);
    let Some(f) = d.navs.get(nav_id) else {
        return server_content.to_string();
    };

    if f.updated_ms > f.synced_ms {
        f.value.clone()
    } else {
        server_content.to_string()
    }
}

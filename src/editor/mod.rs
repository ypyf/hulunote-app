use crate::api::CreateOrUpdateNavRequest;
use crate::components::hooks::use_random::use_random_id_for;
use crate::components::ui::{Command, CommandItem, CommandList};
use crate::drafts::{
    get_due_unsynced_nav_drafts, get_nav_override, get_unsynced_nav_drafts, mark_nav_sync_failed,
    mark_nav_synced, touch_nav,
};
use crate::models::{Nav, Note};
use crate::state::AppContext;
use crate::util::now_ms;
use crate::wiki::{extract_wiki_links, normalize_roam_page_title, parse_wiki_tokens, WikiToken};
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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

// ---- contenteditable helpers (Phase 9 MVP) ----

fn ce_text(el: &web_sys::HtmlElement) -> String {
    // `innerText` preserves line breaks as the user sees them.
    el.inner_text()
}

fn ce_set_text(el: &web_sys::HtmlElement, s: &str) {
    // Avoid setting HTML; keep plain text only.
    el.set_inner_text(s);
}

// ---- contenteditable structural helpers ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoamDeleteState {
    HasContent,
    OnlySoftBreaks,
    Empty,
}

fn has_any_text_content(s: &str) -> bool {
    // Treat some invisible/bogus chars that browsers may inject into contenteditable
    // (to keep caret positions) as non-content.
    fn is_ignorable(c: char) -> bool {
        matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}')
    }

    s.chars().any(|c| !c.is_whitespace() && !is_ignorable(c))
}

fn effective_semantic_br_count(total_br_count: u32, has_trailing_placeholder_br: bool) -> u32 {
    if has_trailing_placeholder_br {
        total_br_count.saturating_sub(1)
    } else {
        total_br_count
    }
}

fn roam_delete_state(has_any_text: bool, semantic_br_count: u32) -> RoamDeleteState {
    if has_any_text {
        return RoamDeleteState::HasContent;
    }
    if semantic_br_count > 0 {
        RoamDeleteState::OnlySoftBreaks
    } else {
        RoamDeleteState::Empty
    }
}

fn should_persist_nav_id(nav_id: &str) -> bool {
    !nav_id.trim().is_empty() && !is_tmp_nav_id(nav_id)
}

fn ensure_trailing_break(doc: &web_sys::Document, root: &web_sys::Node) -> Option<web_sys::Node> {
    // Remove all existing trailing markers inside this root.
    if let Ok(list) = doc.query_selector_all("br[data-trailing-break='1']") {
        for i in 0..list.length() {
            if let Some(n) = list.get(i) {
                if root.contains(Some(&n)) {
                    let _ = n.parent_node().and_then(|p| p.remove_child(&n).ok());
                }
            }
        }
    }

    let Ok(br) = doc.create_element("br") else {
        return None;
    };
    let _ = br.set_attribute("data-trailing-break", "1");
    let br_node: web_sys::Node = br.unchecked_into();
    let _ = root.append_child(&br_node);
    Some(br_node)
}

fn ce_selection_utf16(el: &web_sys::HtmlElement) -> (u32, u32, u32) {
    let txt = ce_text(el);
    let len = txt.encode_utf16().count() as u32;

    let Some(win) = web_sys::window() else {
        return (0, 0, len);
    };
    let Ok(Some(sel)) = win.get_selection() else {
        return (len, len, len);
    };
    if sel.range_count() == 0 {
        return (len, len, len);
    }

    let Ok(range) = sel.get_range_at(0) else {
        return (len, len, len);
    };

    // Ensure selection is within this editor.
    let root_node: web_sys::Node = el.clone().unchecked_into();
    let container: web_sys::Node = match range.common_ancestor_container() {
        Ok(n) => n,
        Err(_) => return (len, len, len),
    };
    if !root_node.contains(Some(&container)) {
        return (len, len, len);
    }

    // Convert (node, offset) -> text length using a prefix range.
    let prefix = range.clone_range();
    let _ = prefix.select_node_contents(&root_node);
    let start_container = match range.start_container() {
        Ok(n) => n,
        Err(_) => return (len, len, len),
    };
    let start_offset = match range.start_offset() {
        Ok(o) => o,
        Err(_) => return (len, len, len),
    };
    let _ = prefix.set_end(&start_container, start_offset);
    let start = prefix
        .to_string()
        .as_string()
        .unwrap_or_default()
        .encode_utf16()
        .count() as u32;

    let prefix2 = range.clone_range();
    let _ = prefix2.select_node_contents(&root_node);
    let end_container = match range.end_container() {
        Ok(n) => n,
        Err(_) => return (start, start, len),
    };
    let end_offset = match range.end_offset() {
        Ok(o) => o,
        Err(_) => return (start, start, len),
    };
    let _ = prefix2.set_end(&end_container, end_offset);
    let end = prefix2
        .to_string()
        .as_string()
        .unwrap_or_default()
        .encode_utf16()
        .count() as u32;

    (start.min(len), end.min(len), len)
}

fn ce_set_caret_utf16(el: &web_sys::HtmlElement, pos_utf16: u32) {
    // The editor node may already be unmounted when this runs (e.g. delayed focus/selection
    // restoration). Avoid creating a Range from detached nodes.
    if !el.is_connected() {
        return;
    }

    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };

    let txt = ce_text(el);
    let len = txt.encode_utf16().count() as u32;
    let target = pos_utf16.min(len);

    let Ok(range) = doc.create_range() else {
        return;
    };

    // Walk text nodes and treat <br> as a single newline char.
    fn child_index(parent: &web_sys::Node, child: &web_sys::Node) -> Option<u32> {
        let kids = parent.child_nodes();
        for i in 0..kids.length() {
            if let Some(n) = kids.get(i) {
                if n == *child {
                    return Some(i);
                }
            }
        }
        None
    }

    fn walk(node: &web_sys::Node, remaining: &mut i32, out: &mut Option<(web_sys::Node, u32)>) {
        if out.is_some() {
            return;
        }

        if node.node_type() == web_sys::Node::TEXT_NODE {
            let s = node.node_value().unwrap_or_default();
            let n = s.encode_utf16().count() as i32;
            if *remaining <= n {
                *out = Some((node.clone(), (*remaining).max(0) as u32));
            } else {
                *remaining -= n;
            }
            return;
        }

        if let Some(el) = node.dyn_ref::<web_sys::Element>() {
            if el.tag_name().to_ascii_lowercase() == "br" {
                if *remaining <= 1 {
                    // Put caret right after the <br>.
                    let Some(parent) = node.parent_node() else {
                        return;
                    };
                    if let Some(idx) = child_index(&parent, node) {
                        *out = Some((parent, idx + 1));
                    }
                } else {
                    *remaining -= 1;
                }
                return;
            }
        }

        let kids = node.child_nodes();
        for i in 0..kids.length() {
            if let Some(k) = kids.get(i) {
                walk(&k, remaining, out);
                if out.is_some() {
                    return;
                }
            }
        }
    }

    let mut remaining = target as i32;
    let mut found: Option<(web_sys::Node, u32)> = None;
    let root_node: web_sys::Node = el.clone().unchecked_into();
    walk(&root_node, &mut remaining, &mut found);

    if let Some((node, off)) = found {
        let _ = range.set_start(&node, off);
        let _ = range.collapse_with_to_start(true);

        if let Ok(Some(sel)) = doc.get_selection() {
            let _ = sel.remove_all_ranges();
            // `addRange()` throws if the range references nodes that are no longer in the document.
            let _ = sel.add_range(&range);
        }
    }
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

    // Treat empty title lists as a valid loaded state.
    // Otherwise, when the backend returns no navs/notes, we would refetch on every keystroke.
    if ac.titles_cache_db.get_untracked().as_deref() == Some(db_id.as_str()) {
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

/// Insert a soft line break at the current selection inside a contenteditable element.
///
/// This uses DOM Selection/Range APIs so caret movement is handled by the browser.
/// Returns true if we inserted a break, false otherwise.
pub(crate) fn should_exit_edit_on_focusout_related_target(
    related: Option<web_sys::EventTarget>,
) -> bool {
    let Some(t) = related else {
        return false;
    };
    let Ok(el) = t.dyn_into::<web_sys::Element>() else {
        return false;
    };

    // If focus stays within outline editor, do NOT exit.
    el.closest(".outline-editor").ok().flatten().is_none()
}

pub(crate) fn should_exit_edit_on_mousedown_target(target: Option<web_sys::EventTarget>) -> bool {
    let Some(t) = target else {
        return false;
    };
    let Ok(el) = t.dyn_into::<web_sys::Element>() else {
        return false;
    };

    // Clicking inside an editor ([data-nav-id] contenteditable) should NOT exit.
    el.closest("[data-nav-id]").ok().flatten().is_none()
}

pub(crate) fn insert_soft_line_break_dom(input_el: &web_sys::HtmlElement) -> bool {
    let _ = input_el.focus();

    let Some(win) = web_sys::window() else {
        return false;
    };
    let Some(doc) = win.document() else {
        return false;
    };

    // Selection can momentarily be unavailable in some browsers/states.
    // Prefer window.getSelection and retry once after focusing.
    let mut sel = win.get_selection().ok().flatten();
    if sel.is_none() {
        let _ = input_el.focus();
        sel = win
            .get_selection()
            .ok()
            .flatten()
            .or_else(|| doc.get_selection().ok().flatten());
    }
    let Some(sel) = sel else {
        return false;
    };

    let root: web_sys::Node = input_el.clone().unchecked_into();

    // Ensure we have a trailing break before insertion.
    let trailing = ensure_trailing_break(&doc, &root);

    // Prefer the current selection range if it exists and is inside this editor.
    // Otherwise, synthesize a range at the end of this editor.
    let mut range = if sel.range_count() > 0 {
        sel.get_range_at(0).ok()
    } else {
        None
    };

    let container_ok = range
        .as_ref()
        .and_then(|r| r.common_ancestor_container().ok())
        .map(|n| root.contains(Some(&n)))
        .unwrap_or(false);

    if !container_ok {
        let Ok(r) = doc.create_range() else {
            return false;
        };
        let _ = r.select_node_contents(&root);
        let _ = r.collapse_with_to_start(false);
        let _ = sel.remove_all_ranges();
        let _ = sel.add_range(&r);
        range = Some(r);
    }

    let Some(range) = range else {
        return false;
    };

    let _ = range.delete_contents();

    // Insert semantic <br> (user content).
    let Ok(br) = doc.create_element("br") else {
        return false;
    };
    let br_node: web_sys::Node = br.unchecked_into();
    let _ = range.insert_node(&br_node);

    // Re-normalize: keep exactly one trailing break at the end.
    let trailing = ensure_trailing_break(&doc, &root).or(trailing);

    // Place caret right after the semantic <br>.
    let _ = range.set_start_after(&br_node);
    let _ = range.collapse_with_to_start(true);

    // If we inserted at the end (i.e. immediately before the trailing placeholder),
    // put caret BEFORE the trailing placeholder so the next Shift+Enter inserts a new
    // semantic <br> (instead of getting stuck on the same line due to browser quirks).
    if let Some(t) = trailing {
        // Determine whether br_node is effectively the last real node before trailing,
        // allowing for empty text nodes that some browsers insert.
        fn is_empty_text_node(n: &web_sys::Node) -> bool {
            n.node_type() == web_sys::Node::TEXT_NODE
                && n.text_content().unwrap_or_default().trim().is_empty()
        }

        let mut cur = t.previous_sibling();
        while let Some(n) = cur.clone() {
            if is_empty_text_node(&n) {
                cur = n.previous_sibling();
                continue;
            }
            break;
        }

        if cur == Some(br_node.clone()) {
            if let Ok(r2) = doc.create_range() {
                let _ = r2.set_start_before(&t);
                let _ = r2.collapse_with_to_start(true);
                let _ = sel.remove_all_ranges();
                let _ = sel.add_range(&r2);
                return true;
            }
        }
    }

    let _ = sel.remove_all_ranges();
    let _ = sel.add_range(&range);

    true
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

pub(crate) fn is_ancestor_of(all: &[Nav], ancestor_id: &str, node_id: &str) -> bool {
    if ancestor_id == node_id {
        return true;
    }

    // Walk up the parent chain from node_id to root.
    let mut cur = node_id;
    for _ in 0..2048 {
        let Some(n) = all.iter().find(|n| n.id == cur) else {
            return false;
        };
        if n.parid == ancestor_id {
            return true;
        }
        if n.parid.trim().is_empty() {
            return false;
        }
        cur = &n.parid;
    }

    false
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

    // Wiki links: opening a missing page does not hit the backend (client-side navigation).

    // Editing state
    let editing_id: RwSignal<Option<String>> = RwSignal::new(None);
    let editing_value: RwSignal<String> = RwSignal::new(String::new());
    // Snapshot of the content when we entered edit mode (id, content).
    // Used to avoid redundant backend saves when the user didn't change anything.
    let editing_snapshot: RwSignal<Option<(String, String)>> = RwSignal::new(None);

    // Drag state (for highlighting drop targets only while dragging).
    let dragging_nav_id: RwSignal<Option<String>> = RwSignal::new(None);
    let drag_over_nav_id: RwSignal<Option<String>> = RwSignal::new(None);

    let target_cursor_col: RwSignal<Option<u32>> = RwSignal::new(None);
    let editing_ref: NodeRef<html::Div> = NodeRef::new();

    // Autocomplete for `[[...]]` (wiki-style)
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

    // Focus handled by OutlineNode (see below).
    // (focus moved to OutlineNode)

    // Debounced autosave for nav drafts (per-nav).
    // - on:input writes local drafts immediately (localStorage)
    // - debounce flush persists the current nav draft to backend
    let autosave_ms: i32 = 1200;
    let autosave_timers: StoredValue<Arc<Mutex<HashMap<String, i32>>>> =
        StoredValue::new(Arc::new(Mutex::new(HashMap::new())));

    let note_id_fn = note_id.clone();

    // Flush all unsynced nav drafts for this note (used for pagehide / exit).
    let flush_note_drafts = StoredValue::new(move || {
        let db_id = app_state
            .0
            .current_database_id
            .get_untracked()
            .unwrap_or_default();
        let note_id_now = note_id_fn();
        if db_id.trim().is_empty() || note_id_now.trim().is_empty() {
            return;
        }

        let api_client = app_state.0.api_client.get_untracked();
        let drafts = get_unsynced_nav_drafts(&db_id, &note_id_now);
        if drafts.is_empty() {
            return;
        }

        spawn_local(async move {
            for (nav_id, content, updated_ms) in drafts {
                let req = CreateOrUpdateNavRequest {
                    note_id: note_id_now.clone(),
                    id: Some(nav_id.clone()),
                    parid: None,
                    content: Some(content),
                    order: None,
                    is_display: None,
                    is_delete: None,
                    properties: None,
                };

                if api_client.upsert_nav(req).await.is_ok() {
                    mark_nav_synced(&db_id, &note_id_now, &nav_id, updated_ms);
                } else {
                    mark_nav_sync_failed(&db_id, &note_id_now, &nav_id);
                }
            }
        });
    });

    // Flush one nav draft.
    let note_id_fn2 = note_id.clone();
    let flush_nav_draft = StoredValue::new(move |nav_id: String| {
        let db_id = app_state
            .0
            .current_database_id
            .get_untracked()
            .unwrap_or_default();
        let note_id_now = note_id_fn2();
        if db_id.trim().is_empty() || note_id_now.trim().is_empty() || nav_id.trim().is_empty() {
            return;
        }

        // Read from localStorage drafts (source of truth).
        let Some((_, content, updated_ms)) = get_unsynced_nav_drafts(&db_id, &note_id_now)
            .into_iter()
            .find(|(id, _, _)| id == &nav_id)
        else {
            return;
        };

        let api_client = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            let req = CreateOrUpdateNavRequest {
                note_id: note_id_now.clone(),
                id: Some(nav_id.clone()),
                parid: None,
                content: Some(content),
                order: None,
                is_display: None,
                is_delete: None,
                properties: None,
            };

            if api_client.upsert_nav(req).await.is_ok() {
                mark_nav_synced(&db_id, &note_id_now, &nav_id, updated_ms);
            } else {
                mark_nav_sync_failed(&db_id, &note_id_now, &nav_id);
            }
        });
    });

    let schedule_autosave = StoredValue::new(move |nav_id: String| {
        if nav_id.trim().is_empty() {
            return;
        }

        let Some(win) = web_sys::window() else {
            return;
        };

        let timers = autosave_timers.get_value();
        if let Ok(mut map) = timers.lock() {
            if let Some(tid) = map.remove(&nav_id) {
                let _ = win.clear_timeout_with_handle(tid);
            }
        }

        let flush = flush_nav_draft;
        let nav_id2 = nav_id.clone();
        let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
            flush.with_value(|f| f(nav_id2));
        });

        let tid = win
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                autosave_ms,
            )
            .unwrap_or(0);
        if let Ok(mut map) = timers.lock() {
            map.insert(nav_id, tid);
        };
    });

    let schedule_autosave_cb: Callback<String> = Callback::new(move |nav_id: String| {
        schedule_autosave.with_value(|f| f(nav_id));
    });

    // Click-to-exit: clicking on non-editable blank space exits editing mode.
    let _mouse_handle = window_event_listener(ev::mousedown, move |ev: web_sys::MouseEvent| {
        if editing_id.try_get_untracked().flatten().is_none() {
            return;
        }

        if should_exit_edit_on_mousedown_target(ev.target()) {
            // Best-effort flush before exiting.
            let _ = flush_note_drafts.try_with_value(|f| f());
            editing_id.set(None);
            editing_snapshot.set(None);
        }
    });

    // Retry queue worker (standard local-first): periodically flush due drafts.
    // Uses `retry_count/next_retry_ms` to back off after failures.
    let retry_interval_ms: i32 = 2000;
    let retry_timer_id: RwSignal<Option<i32>> = RwSignal::new(None);

    let note_id_fn3 = note_id.clone();
    let retry_tick = StoredValue::new(move || {
        let db_id = app_state
            .0
            .current_database_id
            .get_untracked()
            .unwrap_or_default();
        let note_id_now = note_id_fn3();
        if db_id.trim().is_empty() || note_id_now.trim().is_empty() {
            return;
        }

        let due = get_due_unsynced_nav_drafts(&db_id, &note_id_now, now_ms(), 2);
        if due.is_empty() {
            return;
        }

        let api_client = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            for (nav_id, content, updated_ms) in due {
                let req = CreateOrUpdateNavRequest {
                    note_id: note_id_now.clone(),
                    id: Some(nav_id.clone()),
                    parid: None,
                    content: Some(content),
                    order: None,
                    is_display: None,
                    is_delete: None,
                    properties: None,
                };

                if api_client.upsert_nav(req).await.is_ok() {
                    mark_nav_synced(&db_id, &note_id_now, &nav_id, updated_ms);
                } else {
                    mark_nav_sync_failed(&db_id, &note_id_now, &nav_id);
                }
            }
        });
    });

    // Start retry timer.
    Effect::new(move |_| {
        // start once
        if retry_timer_id.get_untracked().is_some() {
            return;
        }

        let Some(win) = web_sys::window() else {
            return;
        };

        let tick = retry_tick;
        let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
            let _ = tick.try_with_value(|f| f());
        }) as Box<dyn FnMut()>);

        let tid = win
            .set_interval_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                retry_interval_ms,
            )
            .unwrap_or(0);
        retry_timer_id.set(Some(tid));

        // Stop the interval when the editor unmounts to avoid calling disposed reactive values.
        on_cleanup(move || {
            if let Some(win) = web_sys::window() {
                let _ = win.clear_interval_with_handle(tid);
            }
        });

        cb.forget();
    });

    // Kick retry tick on reconnect.
    let _online_handle = window_event_listener(ev::online, move |_ev: web_sys::Event| {
        let _ = retry_tick.try_with_value(|f| f());
    });

    // Best-effort flush on page hide (refresh/close/navigation) — keep it beacon/keepalive-friendly.
    let note_id_fn4 = note_id.clone();
    let _pagehide_handle =
        window_event_listener(ev::pagehide, move |_ev: web_sys::PageTransitionEvent| {
            let k_recent: usize = 5;

            let db_id = app_state
                .0
                .current_database_id
                .get_untracked()
                .unwrap_or_default();
            let note_id_now = note_id_fn4();
            if db_id.trim().is_empty() || note_id_now.trim().is_empty() {
                return;
            }

            let mut drafts = get_unsynced_nav_drafts(&db_id, &note_id_now);
            if drafts.is_empty() {
                return;
            }

            drafts.sort_by(|a, b| b.2.cmp(&a.2));

            let editing = editing_id.try_get_untracked().flatten();
            let mut picked: Vec<(String, String, i64)> = Vec::new();

            if let Some(editing_nav) = editing {
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

            let api_client = app_state.0.api_client.get_untracked();
            spawn_local(async move {
                for (nav_id, content, updated_ms) in picked {
                    let req = CreateOrUpdateNavRequest {
                        note_id: note_id_now.clone(),
                        id: Some(nav_id.clone()),
                        parid: None,
                        content: Some(content),
                        order: None,
                        is_display: None,
                        is_delete: None,
                        properties: None,
                    };

                    if api_client.upsert_nav(req).await.is_ok() {
                        mark_nav_synced(&db_id, &note_id_now, &nav_id, updated_ms);
                    } else {
                        mark_nav_sync_failed(&db_id, &note_id_now, &nav_id);
                    }
                }
            });
        });

    // Keep the contenteditable DOM in sync when switching nodes.
    // IMPORTANT: do not re-apply on every keystroke (would break IME / caret).
    Effect::new(move |_| {
        let id = editing_id.get();
        if id.is_none() {
            return;
        }
        let el = editing_ref.get();
        if let Some(el) = el {
            let he: web_sys::HtmlElement = el.unchecked_into();
            ce_set_text(&he, &editing_value.get_untracked());
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
        <div class="rounded-md p-3">

            // NOTE: intentionally no loading spinner when switching notes.

            <Show when=move || error.get().is_some() fallback=|| ().into_view()>
                {move || error.get().map(|e| view! {
                    <div class="mt-2 text-xs text-destructive">{e}</div>
                })}
            </Show>

            // Opening missing pages does not show an error banner here.

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
                                                editing_snapshot=editing_snapshot
                                                dragging_nav_id=dragging_nav_id
                                                drag_over_nav_id=drag_over_nav_id
                                                target_cursor_col=target_cursor_col
                                                editing_ref=editing_ref
                                                focused_nav_id=focused_nav_id
                                                schedule_autosave=schedule_autosave_cb
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
    editing_snapshot: RwSignal<Option<(String, String)>>,
    dragging_nav_id: RwSignal<Option<String>>,
    drag_over_nav_id: RwSignal<Option<String>>,
    target_cursor_col: RwSignal<Option<u32>>,
    editing_ref: NodeRef<html::Div>,
    focused_nav_id: RwSignal<Option<String>>,
    schedule_autosave: Callback<String>,
) -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let ac = expect_context::<AutocompleteCtx>();
    let navigate = leptos_router::hooks::use_navigate();

    // Capture autocomplete signals directly for event handlers that may fire after unmount (e.g. blur).
    // Avoid accessing `StoredValue` in those cases because it may have been disposed.
    let ac_open = ac.ac_open;
    let ac_start_utf16 = ac.ac_start_utf16;

    // IME stability: while composing, don't intercept outliner keys like Enter/Tab.
    let is_composing: RwSignal<bool> = RwSignal::new(false);

    let nav_id_for_nav = nav_id.clone();
    let nav_id_for_toggle = nav_id.clone();
    let nav_id_for_render = nav_id.clone();
    let note_id_for_toggle = note_id.clone();

    // (handler ids are captured per-render; avoid moving values out of the render closure)

    let nav_id_sv = StoredValue::new(nav_id.clone());

    // Focus is handled at the node level (instead of OutlineEditor + setTimeout) so the component
    // that owns the contenteditable DOM is responsible for focusing it.
    Effect::new(move |_| {
        let my_id = nav_id_sv.get_value();
        let is_editing = editing_id.get().as_deref() == Some(my_id.as_str());
        if !is_editing {
            return;
        }

        let col = target_cursor_col.get_untracked();
        let editing_ref2 = editing_ref.clone();

        // Defer to the next animation frame so the contenteditable element is mounted and the
        // NodeRef is populated, without accumulating unbounded setTimeout callbacks.
        let _ = web_sys::window().unwrap().request_animation_frame(
            wasm_bindgen::closure::Closure::once_into_js(move || {
                // This callback runs outside reactive tracking; use untracked access.
                let Some(el) = editing_ref2.get_untracked() else {
                    return;
                };

                let _ = el.focus();
                if let Some(col) = col {
                    let he: web_sys::HtmlElement = el.unchecked_into();
                    ce_set_caret_utf16(&he, col);
                }
            })
            .as_ref()
            .unchecked_ref(),
        );
    });
    let note_id_sv = StoredValue::new(note_id.clone());
    let app_state_sv = StoredValue::new(app_state.clone());
    let ac_sv = StoredValue::new(ac.clone());
    let navigate_sv = StoredValue::new(navigate.clone());

    // Stable ids for the `[[...]]` autocomplete popover (anchor positioning).
    let ac_uid_sv = StoredValue::new(use_random_id_for("ac_menu"));
    let ac_popover_id_sv = StoredValue::new(format!("ac_popover{}", ac_uid_sv.get_value()));
    let ac_anchor_name_sv = StoredValue::new(format!("--ac_anchor{}", ac_uid_sv.get_value()));

    // Autocomplete list container ref (for keyboard selection scroll).
    let ac_list_ref: NodeRef<html::Div> = NodeRef::new();

    // Keep selected item visible while navigating the autocomplete menu with ArrowUp/ArrowDown.
    Effect::new(move |_| {
        let ac = ac_sv.get_value();
        if !ac.ac_open.get() {
            return;
        }

        // Track both items and index so we react to changes.
        let items_len = ac.ac_items.get().len();
        let _idx = ac.ac_index.get();
        if items_len == 0 {
            return;
        }

        let Some(list_el) = ac_list_ref.get() else {
            return;
        };

        // Defer to next tick so DOM updates have applied.
        let _ = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                wasm_bindgen::closure::Closure::once_into_js(move || {
                    let list_elem: web_sys::Element = list_el.unchecked_into();
                    let Ok(Some(row)) =
                        list_elem.query_selector("[data-name='CommandItem'][aria-selected='true']")
                    else {
                        return;
                    };

                    let list_he: web_sys::HtmlElement = list_elem.unchecked_into();
                    let row_he: web_sys::HtmlElement = row.unchecked_into();

                    let row_top = row_he.offset_top() as i32;
                    let row_bottom = row_top + row_he.offset_height() as i32;

                    let view_top = list_he.scroll_top();
                    let view_bottom = view_top + list_he.client_height() as i32;

                    if row_top < view_top {
                        list_he.set_scroll_top(row_top);
                    } else if row_bottom > view_bottom {
                        list_he.set_scroll_top(row_bottom - list_he.client_height() as i32);
                    }
                })
                .as_ref()
                .unchecked_ref(),
                0,
            );
    });

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

                    let schedule_autosave = schedule_autosave;
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
                                        editing_snapshot=editing_snapshot
                                        dragging_nav_id=dragging_nav_id
                                        drag_over_nav_id=drag_over_nav_id
                                        target_cursor_col=target_cursor_col
                                        editing_ref=editing_ref
                                        focused_nav_id=focused_nav_id
                                        schedule_autosave=schedule_autosave
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
                                    let _is_focused = focused_nav_id.get().as_deref() == Some(id.as_str());

                                    let is_dragging = dragging_nav_id.get().is_some();
                                    let is_drag_source = dragging_nav_id.get().as_deref() == Some(id.as_str());
                                    let is_drag_over = drag_over_nav_id.get().as_deref() == Some(id.as_str());

                                    if is_editing {
                                        "outline-row outline-row--editing flex items-center gap-2 py-1"
                                    } else if is_dragging && is_drag_source {
                                        // Make the dragged row semi-transparent (keep content visible).
                                        "outline-row flex items-center gap-2 py-1 rounded-md bg-muted/30 opacity-40"
                                    } else if is_dragging && is_drag_over {
                                        // Highlight drop target only while dragging.
                                        "outline-row flex items-center gap-2 py-1 rounded-md bg-muted ring-1 ring-ring/40"
                                    } else {
                                        "outline-row flex items-center gap-2 py-1"
                                    }
                                }
                                // Drag is started from the bullet/triangle only (button below).
                                on:dragenter=move |ev: web_sys::DragEvent| {
                                    let target_id = nav_id_sv.get_value();
                                    let dragged_id = dragging_nav_id.get_untracked().unwrap_or_default();

                                    // Disallow dropping a node into its own subtree.
                                    if !dragged_id.trim().is_empty()
                                        && is_ancestor_of(&navs.get_untracked(), &dragged_id, &target_id)
                                    {
                                        drag_over_nav_id.set(None);
                                        if let Some(dt) = ev.data_transfer() {
                                            dt.set_drop_effect("none");
                                        }
                                        return;
                                    }

                                    ev.prevent_default();
                                    drag_over_nav_id.set(Some(target_id));
                                }
                                on:dragover=move |ev: web_sys::DragEvent| {
                                    let target_id = nav_id_sv.get_value();
                                    let dragged_id = dragging_nav_id.get_untracked().unwrap_or_default();

                                    // Disallow dropping a node into its own subtree.
                                    if !dragged_id.trim().is_empty()
                                        && is_ancestor_of(&navs.get_untracked(), &dragged_id, &target_id)
                                    {
                                        drag_over_nav_id.set(None);
                                        if let Some(dt) = ev.data_transfer() {
                                            dt.set_drop_effect("none");
                                        }
                                        return;
                                    }

                                    ev.prevent_default();
                                    drag_over_nav_id.set(Some(target_id));
                                    if let Some(dt) = ev.data_transfer() {
                                        dt.set_drop_effect("move");
                                    }
                                }
                                on:dragleave=move |_ev: web_sys::DragEvent| {
                                    // Best-effort: clear highlight when leaving this row.
                                    // The next dragenter/dragover will set it again.
                                    if drag_over_nav_id.get_untracked().as_deref() == Some(nav_id_sv.get_value().as_str()) {
                                        drag_over_nav_id.set(None);
                                    }
                                }
                                on:drop=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default();

                                    // Read dragged_id before clearing drag state.
                                    let dragged_id = dragging_nav_id.get_untracked().unwrap_or_default();
                                    if dragged_id.trim().is_empty() {
                                        return;
                                    }

                                    // Disallow dropping a node into its own subtree.
                                    if is_ancestor_of(&navs.get_untracked(), &dragged_id, &nav_id_sv.get_value()) {
                                        return;
                                    }
                                    if is_tmp_nav_id(&dragged_id) {
                                        return;
                                    }

                                    // Drop completes the drag: clear drag state immediately so UI restores.
                                    dragging_nav_id.set(None);
                                    drag_over_nav_id.set(None);

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
                                draggable="true"
                                on:dragstart=move |ev: web_sys::DragEvent| {
                                    let id = nav_id_sv.get_value();

                                    // UX: dragging should not keep the row in editing state.
                                    if editing_id.get_untracked().as_deref() == Some(id.as_str()) {
                                        editing_id.set(None);
                                        // Close autocomplete if it was open.
                                        ac.ac_open.set(false);
                                        ac.ac_start_utf16.set(None);
                                    }

                                    dragging_nav_id.set(Some(id.clone()));
                                    drag_over_nav_id.set(Some(id.clone()));

                                    if let Some(dt) = ev.data_transfer() {
                                        let _ = dt.set_data("text/plain", &id);
                                        dt.set_drop_effect("move");

                                        // Show the whole row as the drag preview (not just the bullet).
                                        if let Some(row) = ev
                                            .current_target()
                                            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                                            .and_then(|el| el.closest(".outline-row").ok().flatten())
                                        {
                                            // Anchor the drag preview under the cursor to avoid the “jump” feeling.
                                            let rect = row.get_bounding_client_rect();
                                            let ox = ((ev.client_x() as f64) - rect.left()).round() as i32;
                                            let oy = ((ev.client_y() as f64) - rect.top()).round() as i32;
                                            dt.set_drag_image(&row, ox, oy);
                                        }
                                    }
                                }
                                on:dragend=move |_ev: web_sys::DragEvent| {
                                    dragging_nav_id.set(None);
                                    drag_over_nav_id.set(None);
                                }
                                on:click={
                                    let has_kids = has_kids;
                                    move |ev| {
                                        if has_kids {
                                            on_toggle_cb.run(ev)
                                        }
                                    }
                                }
                                aria-disabled=move || (!has_kids).to_string()
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
                                        // When not editing, still reflect local-first drafts stored in localStorage.
                                        // Otherwise a refresh shows stale server content until the user re-enters edit mode.
                                        let db_id = app_state.0.current_database_id.get_untracked().unwrap_or_default();
                                        let note_id = note_id_sv.get_value();
                                        let id_now = nav_id_sv.get_value();
                                        let content_now = get_nav_override(&db_id, &note_id, &id_now, &n.content);
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
                                                class="cursor-text whitespace-pre-wrap min-h-[28px] px-3 py-1"
                                                on:mousedown=move |_ev: web_sys::MouseEvent| {
                                                    // Use mousedown (not click) for single-click switching.
                                                    // IMPORTANT: don't rely on `blur` to save. When a focused input is
                                                    // unmounted by state updates, browsers may not fire blur reliably.
                                                    // Save the current editing buffer explicitly before switching.

                                                    if let Some(current_id) = editing_id.get_untracked() {
                                                        // IMPORTANT: when the editor surface is contenteditable, the DOM
                                                        // can be ahead of our signal (e.g. certain edit operations).
                                                        // Read from the DOM when possible.
                                                        let current_content = editing_ref
                                                            .get_untracked()
                                                            .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
                                                            .map(|el| ce_text(&el))
                                                            .unwrap_or_else(|| editing_value.get_untracked());

                                                        // Update local state.
                                                        navs.update(|xs| {
                                                            let _ = apply_nav_content(xs, &current_id, &current_content);
                                                        });

                                                        // Persist to backend only if content changed since we entered edit mode.
                                                        let should_save = editing_snapshot
                                                            .get_untracked()
                                                            .filter(|(id, _)| id == &current_id)
                                                            .map(|(_id, original)| original != current_content)
                                                            .unwrap_or_else(|| {
                                                                // Fallback: compare against current nav content.
                                                                get_nav_content(&navs.get_untracked(), &current_id).unwrap_or_default() != current_content
                                                            });

                                                        if should_save {
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
                                                    }

                                                    // Defer the actual switch so the current input can unmount cleanly.
                                                    let id = id_for_click.clone();
                                                    let next_value = content_for_click.clone();
                                                    let editing_id = editing_id;
                                                    let editing_value = editing_value;
                                                    let editing_snapshot = editing_snapshot;
                                                    let target_cursor_col = target_cursor_col;

                                                    let db_id = app_state.0.current_database_id.get_untracked().unwrap_or_default();
                                                    let note_id = note_id_sv.get_value();

                                                    let cb = Closure::<dyn FnMut()>::new(move || {
                                                        let restored = get_nav_override(&db_id, &note_id, &id, &next_value);

                                                        editing_id.set(Some(id.clone()));
                                                        editing_value.set(restored.clone());
                                                        editing_snapshot.set(Some((id.clone(), restored.clone())));
                                                        // Default caret position: end of content.
                                                        target_cursor_col.set(Some(restored.encode_utf16().count() as u32));
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
                                                                    // Use native Popover API + CSS Anchor Positioning (same tech as Rust/UI Popover),
                                                                    // but wire it for hover + interactive content.
                                                                    let preview_loading: RwSignal<bool> = RwSignal::new(false);
                                                                    let preview_error: RwSignal<Option<String>> = RwSignal::new(None);
                                                                    let preview_lines: RwSignal<Vec<String>> = RwSignal::new(vec![]);
                                                                    let preview_loaded_for: RwSignal<Option<String>> = RwSignal::new(None);

                                                                    let title_for_hover = title_raw.clone();

                                                                    let preview_uid = use_random_id_for("wiki_preview");
                                                                    let preview_trigger_id = format!("wiki_preview_trigger{}", preview_uid);
                                                                    let preview_popover_id = format!("wiki_preview_popover{}", preview_uid);
                                                                    let preview_anchor_name = format!("--wiki_preview_anchor{}", preview_uid);

                                                                    let preview_script = format!(
                                                                        r#"(() => {{
  const trigger = document.getElementById('{trigger_id}');
  const pop = document.getElementById('{popover_id}');
  if (!trigger || !pop || pop.dataset.init) return;
  pop.dataset.init = '1';

  let hideTimer = null;
  const show = () => {{
    if (hideTimer) {{ clearTimeout(hideTimer); hideTimer = null; }}
    if (!pop.matches(':popover-open')) pop.showPopover();
  }};
  const hideSoon = () => {{
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {{
      // Only hide if neither trigger nor popover is hovered.
      if (!trigger.matches(':hover') && !pop.matches(':hover')) {{
        try {{ pop.hidePopover(); }} catch (_) {{}}
      }}
    }}, 80);
  }};

  trigger.addEventListener('mouseenter', show);
  trigger.addEventListener('mouseleave', hideSoon);
  pop.addEventListener('mouseenter', show);
  pop.addEventListener('mouseleave', hideSoon);
}})();"#,
                                                                        trigger_id = preview_trigger_id,
                                                                        popover_id = preview_popover_id,
                                                                    );

                                                                    view! {
                                                                        <>
                                                                            <style>
                                                                                {format!(
                                                                                    r#"
#{popover_id} {{
  position-anchor: {anchor_name};
  inset: auto;
  top: anchor(bottom);
  left: anchor(left);
  margin-top: 8px;
  @position-try(flip-block) {{
    bottom: anchor(top);
    top: auto;
    margin-bottom: 8px;
    margin-top: 0;
  }}
  position-try-fallbacks: flip-block;
  position-try-order: most-height;
  position-visibility: anchors-visible;
  z-index: 1000000;
}}
"#,
                                                                                    popover_id = preview_popover_id,
                                                                                    anchor_name = preview_anchor_name
                                                                                )}
                                                                            </style>

                                                                            <button
                                                                                id=preview_trigger_id
                                                                                type="button"
                                                                                class="cursor-pointer text-primary underline underline-offset-2 hover:text-primary/80"
                                                                                style=format!("anchor-name: {}", preview_anchor_name)
                                                                                on:mouseenter=move |_ev: web_sys::MouseEvent| {
                                                                                    // Lazy-load preview data.
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
                                                                                        let mut note_id_opt = notes
                                                                                            .iter()
                                                                                            .find(|n| {
                                                                                                n.database_id == db_id
                                                                                                    && normalize_roam_page_title(&n.title) == title_norm
                                                                                            })
                                                                                            .map(|n| n.id.clone());

                                                                                        if note_id_opt.is_none() {
                                                                                            match api_client.get_all_note_list(&db_id).await {
                                                                                                Ok(notes2) => {
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
                                                                                            preview_loading.set(false);
                                                                                            return;
                                                                                        };

                                                                                        match api_client.get_note_navs(&note_id).await {
                                                                                            Ok(navs) => {
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
                                                                                                        out.push(format!("{}{}", indent, n.content));
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
                                                                                on:mousedown=move |ev: web_sys::MouseEvent| {
                                                                                    // Keep existing navigation behavior (left click only).
                                                                                    if ev.button() != 0 {
                                                                                        return;
                                                                                    }
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

                                                                                        if let Some(id) = find_existing_id(&app_state2.0.notes.get_untracked()) {
                                                                                            navigate2(
                                                                                                &format!("/db/{}/note/{}", db_id, id),
                                                                                                leptos_router::NavigateOptions::default(),
                                                                                            );
                                                                                            return;
                                                                                        }

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
                                                                            </button>

                                                                            <div
                                                                                id=preview_popover_id
                                                                                popover="manual"
                                                                                class="w-[28rem] max-w-[90vw] rounded-md border border-border-strong bg-card text-card-foreground p-3 text-xs shadow-lg"
                                                                            >
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

                                                                            <script>{preview_script}</script>
                                                                        </>
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
                                        <div
                                            node_ref=editing_ref
                                            contenteditable="true"
                                            role="textbox"
                                            // Store stable ids on the DOM node so blur handlers can read them even if
                                            // reactive values are disposed during navigation/unmount.
                                            attr:data-nav-id=nav_id_sv.get_value()
                                            attr:data-note-id=note_id_sv.get_value()
                                            style=format!("anchor-name: {}", ac_anchor_name_sv.get_value())
                                            class="min-h-[28px] w-full min-w-0 flex-1 rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50 whitespace-pre-wrap"
                                            on:input=move |ev: web_sys::Event| {
                                                let Some(el) = ev
                                                    .target()
                                                    .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                                                else {
                                                    return;
                                                };

                                                let v = ce_text(&el);
                                                editing_value.set(v.clone());

                                                // Local-first: store draft at note-level aggregate.
                                                let db_id = app_state_sv
                                                    .get_value()
                                                    .0
                                                    .current_database_id
                                                    .get_untracked()
                                                    .unwrap_or_default();
                                                let note_id = note_id_sv.get_value();
                                                let nav_id = nav_id_sv.get_value();
                                                touch_nav(&db_id, &note_id, &nav_id, &v);
                                                schedule_autosave.run(nav_id.clone());

                                                // Autocomplete: detect an unclosed `[[...` immediately before the caret.
                                                let (caret_utf16, _caret_end_utf16, _len) = ce_selection_utf16(&el);

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
                                            on:compositionstart=move |_ev: web_sys::CompositionEvent| {
                                                is_composing.set(true);
                                            }
                                            on:compositionend=move |ev: web_sys::CompositionEvent| {
                                                is_composing.set(false);
                                                if let Some(el) = ev
                                                    .target()
                                                    .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                                                {
                                                    editing_value.set(ce_text(&el));
                                                }
                                            }
                                            // on:blur only persists content; it does NOT decide whether we should exit
                                            // editing mode (that decision belongs to focusout/relatedTarget).
                                            on:blur={
                                                let nav_id_fallback = nav_id_sv.get_value();
                                                let note_id_fallback = note_id_sv.get_value();
                                                move |ev| {
                                                    // Close autocomplete if open.
                                                    ac_open.set(false);
                                                    ac_start_utf16.set(None);

                                                    let Some(el) = ev
                                                        .current_target()
                                                        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                                                        .or_else(|| {
                                                            ev.target()
                                                                .and_then(|t| {
                                                                    t.dyn_into::<web_sys::HtmlElement>().ok()
                                                                })
                                                                .and_then(|t| {
                                                                    t.closest("[data-nav-id]")
                                                                        .ok()
                                                                        .flatten()
                                                                        .and_then(|e| {
                                                                            e.dyn_into::<web_sys::HtmlElement>().ok()
                                                                        })
                                                                })
                                                        })
                                                    else {
                                                        leptos::logging::log!(
                                                            "[editor] blur: no HtmlElement target"
                                                        );
                                                        return;
                                                    };

                                                    // IMPORTANT: read the value from the contenteditable element.
                                                    let new_content = ce_text(&el);

                                                    // Read ids from DOM attributes (component may be disposed during nav).
                                                    let mut nav_id_now =
                                                        el.get_attribute("data-nav-id").unwrap_or_default();
                                                    let mut note_id_now =
                                                        el.get_attribute("data-note-id").unwrap_or_default();

                                                    if nav_id_now.trim().is_empty() {
                                                        nav_id_now = nav_id_fallback.clone();
                                                    }
                                                    if note_id_now.trim().is_empty() {
                                                        note_id_now = note_id_fallback.clone();
                                                    }

                                                    if nav_id_now.trim().is_empty()
                                                        || note_id_now.trim().is_empty()
                                                        || is_tmp_nav_id(&nav_id_now)
                                                    {
                                                        return;
                                                    }

                                                    // MVP: always persist on blur.
                                                    navs.update(|xs| {
                                                        let _ = apply_nav_content(xs, &nav_id_now, &new_content);
                                                    });

                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    let db_id = app_state
                                                        .0
                                                        .current_database_id
                                                        .get_untracked()
                                                        .unwrap_or_default();
                                                    let note_id_now2 = note_id_now.clone();
                                                    let nav_id_now2 = nav_id_now.clone();

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
                                                        if api_client.upsert_nav(req).await.is_ok() {
                                                            mark_nav_synced(
                                                                &db_id,
                                                                &note_id_now2,
                                                                &nav_id_now2,
                                                                now_ms(),
                                                            );
                                                        }
                                                    });
                                                }
                                            }
                                            on:focusout=move |ev: web_sys::FocusEvent| {
                                                if !should_exit_edit_on_focusout_related_target(
                                                    ev.related_target(),
                                                ) {
                                                    return;
                                                }

                                                let nav_id_now = nav_id_sv.get_value();
                                                if editing_id.get_untracked().as_deref() == Some(nav_id_now.as_str()) {
                                                    editing_id.set(None);
                                                    editing_snapshot.set(None);
                                                }
                                            }

                                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                let key = ev.key();

                                                if is_composing.get_untracked() {
                                                    // Don't interfere with IME (Enter/Arrow keys are often used to select candidates).
                                                    return;
                                                }

                                                // Helpers for reading the current contenteditable element.
                                                // Prefer `current_target` (the element the handler is attached to).
                                                let input = || {
                                                    ev.current_target()
                                                        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                                                        .or_else(|| {
                                                            // Fallback: keydown target can be a Text node; walk up to parent element.
                                                            ev.target()
                                                                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                                                                .and_then(|n| n.parent_element())
                                                                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
                                                        })
                                                };

                                                let ac = ac_sv.get_value();

                                                // Autocomplete menu key handling.
                                                // NOTE: allow Shift+Enter to fall through for soft line breaks.
                                                if ac.ac_open.get_untracked() && !(key == "Enter" && ev.shift_key()) {
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
                                                                    let v = ce_text(&input_el);
                                                                    let (caret_utf16, _caret_end_utf16, _len) =
                                                                        ce_selection_utf16(&input_el);

                                                                    let caret_byte = utf16_to_byte_idx(&v, caret_utf16);
                                                                    let start_utf16 =
                                                                        ac.ac_start_utf16.get_untracked().unwrap_or(0);
                                                                    let start_byte = utf16_to_byte_idx(&v, start_utf16);

                                                                    let mut next = String::new();
                                                                    next.push_str(&v[..start_byte.min(v.len())]);
                                                                    next.push_str("[[");
                                                                    next.push_str(&chosen);
                                                                    next.push_str("]]");
                                                                    next.push_str(&v[caret_byte.min(v.len())..]);

                                                                    ce_set_text(&input_el, &next);
                                                                    editing_value.set(next.clone());

                                                                    let caret_after = start_utf16
                                                                        + 2
                                                                        + (chosen.encode_utf16().count() as u32)
                                                                        + 2;
                                                                    ce_set_caret_utf16(&input_el, caret_after);
                                                                }

                                                                ac.ac_open.set(false);
                                                                ac.ac_start_utf16.set(None);
                                                            }
                                                            return;
                                                        }
                                                        _ => {}
                                                    }
                                                }

                                                // Helpers for wiki-style navigation

                                                let save_current = |nav_id_now: &str, note_id_now: &str| {
                                                    let current_content = editing_value.get_untracked();
                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                        }
                                                    });

                                                    // Persist to backend only if content changed since we entered edit mode.
                                                    let should_save = editing_snapshot
                                                        .get_untracked()
                                                        .filter(|(id, _)| id == nav_id_now)
                                                        .map(|(_id, original)| original != current_content)
                                                        .unwrap_or_else(|| {
                                                            // Fallback: compare against current nav content.
                                                            get_nav_content(&navs.get_untracked(), nav_id_now).unwrap_or_default() != current_content
                                                        });

                                                    if should_save {
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
                                                    }
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
                                                        .map(|i| ce_selection_utf16(i).0)
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
                                                        content: Some(current_content.clone()),
                                                        order: Some(new_order),
                                                        is_display: None,
                                                        is_delete: None,
                                                        properties: None,
                                                    };
                                                    spawn_local(async move {
                                                        let _ = api_client.upsert_nav(req).await;
                                                    });

                                                    // Keep editing current node.
                                                    editing_id.set(Some(nav_id_now.clone()));
                                                    editing_snapshot.set(Some((nav_id_now, current_content)));
                                                    return;
                                                }

                                                // Arrow Up/Down: move between visible nodes
                                                if key == "ArrowUp" || key == "ArrowDown" {
                                                    ev.prevent_default();

                                                    let cursor_col = input()
                                                        .as_ref()
                                                        .map(|i| ce_selection_utf16(i).0)
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
                                                            editing_id.set(Some(next_id.clone()));
                                                            editing_value.set(next_nav.content.clone());
                                                            editing_snapshot.set(Some((next_id, next_nav.content.clone())));
                                                        }
                                                    }

                                                    return;
                                                }

                                                // Arrow Left/Right: jump to prev/next visible node at boundaries
                                                if key == "ArrowLeft" || key == "ArrowRight" {
                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    let (cursor_start, cursor_end, len) = if let Some(i) = input() {
                                                        ce_selection_utf16(&i)
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
                                                                    editing_snapshot.set(Some((parent.id.clone(), parent.content.clone())));
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
                                                        editing_snapshot.set(Some((target.id.clone(), target.content.clone())));
                                                        target_cursor_col.set(Some(target.content.encode_utf16().count() as u32));
                                                        return;
                                                    }

                                                    if key == "ArrowRight" && cursor_start == len {
                                                        ev.prevent_default();
                                                        target_cursor_col.set(None);
                                                        save_current(&nav_id_now, &note_id_now);

                                                        let all = navs.get_untracked();

                                                        // If the current node has children and is collapsed, expand it.
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
                                                                // ArrowRight at end expands and moves into the child branch.
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
                                                                editing_snapshot.set(Some((first_child.id.clone(), first_child.content.clone())));
                                                                target_cursor_col.set(Some(0));
                                                                return;
                                                            }

                                                            // Move into first child.
                                                            editing_id.set(Some(first_child.id.clone()));
                                                            editing_value.set(first_child.content.clone());
                                                            editing_snapshot.set(Some((first_child.id.clone(), first_child.content.clone())));
                                                            target_cursor_col.set(Some(0));
                                                            return;
                                                        }

                                                        // If there are no children, ArrowRight does not move to a sibling.
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
                                                            content: Some(current_content.clone()),
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
                                                            content: Some(current_content.clone()),
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
                                                    editing_id.set(Some(nav_id_now.clone()));
                                                    editing_snapshot.set(Some((nav_id_now, current_content)));
                                                    return;
                                                }

                                                // Backspace/Delete on empty: soft-delete node (and its subtree)
                                                // IMPORTANT: on keydown, `editing_value` may lag behind the DOM.
                                                // Use the live DOM text if available, otherwise fall back to the signal.
                                                let v_now = input()
                                                    .as_ref()
                                                    .map(ce_text)
                                                    .unwrap_or_else(|| editing_value.get_untracked());

                                                // Roam-style delete:
                                                // Roam-style delete (trailing break aware):
                                                // - We maintain a trailing `<br data-trailing-break="1">` placeholder for caret.
                                                //   It is NOT user content.
                                                // - If the node has semantic soft breaks (`<br>` without the marker) but no text,
                                                //   Backspace/Delete removes one break at a time.
                                                // - Once only the trailing placeholder remains (no semantic breaks, no text),
                                                //   Backspace/Delete deletes the node.
                                                let (semantic_br_count, has_any_text) = input()
                                                    .as_ref()
                                                    .and_then(|el| {
                                                        fn is_empty_text_node(n: &web_sys::Node) -> bool {
                                                            n.node_type() == web_sys::Node::TEXT_NODE
                                                                && n.text_content().unwrap_or_default().trim().is_empty()
                                                        }

                                                        let root: web_sys::Node = el.clone().unchecked_into();

                                                        // Find the last non-empty child node.
                                                        let mut last_nonempty: Option<web_sys::Node> = None;
                                                        let kids = root.child_nodes();
                                                        for i in 0..kids.length() {
                                                            if let Some(n) = kids.get(i) {
                                                                if is_empty_text_node(&n) {
                                                                    continue;
                                                                }
                                                                last_nonempty = Some(n);
                                                            }
                                                        }

                                                        let total_br = el
                                                            .query_selector_all("br")
                                                            .ok()
                                                            .map(|l| l.length())
                                                            .unwrap_or(0);

                                                        let has_trailing_placeholder_br = last_nonempty
                                                            .as_ref()
                                                            .and_then(|n| n.dyn_ref::<web_sys::Element>())
                                                            .map(|e| e.tag_name().to_uppercase() == "BR")
                                                            .unwrap_or(false);

                                                        let semantic = effective_semantic_br_count(
                                                            total_br,
                                                            has_trailing_placeholder_br,
                                                        );

                                                        let txt = ce_text(el);
                                                        let has_text = has_any_text_content(&txt);
                                                        Some((semantic, has_text))
                                                    })
                                                    .unwrap_or((0, has_any_text_content(&v_now)));

                                                let state = roam_delete_state(has_any_text, semantic_br_count);

                                                if (key == "Backspace" || key == "Delete")
                                                    && state == RoamDeleteState::OnlySoftBreaks
                                                {
                                                    ev.prevent_default();

                                                    // Remove one semantic soft break at a time.
                                                    // In our model, the trailing placeholder break is always the last BR.
                                                    if let Some(el) = input() {
                                                        if let Ok(list) = el.query_selector_all("br") {
                                                            let len = list.length();
                                                            if len >= 2 {
                                                                // Remove the br right before the trailing placeholder.
                                                                if let Some(to_remove) = list.get(len - 2) {
                                                                    let _ = to_remove
                                                                        .parent_node()
                                                                        .and_then(|p| p.remove_child(&to_remove).ok());
                                                                }
                                                            }
                                                        }

                                                        // Re-normalize trailing placeholder.
                                                        let doc = web_sys::window().and_then(|w| w.document());
                                                        if let Some(doc) = doc {
                                                            let root: web_sys::Node = el.clone().unchecked_into();
                                                            let _ = ensure_trailing_break(&doc, &root);
                                                        }

                                                        // Keep caret at end.
                                                        let txt = ce_text(&el);
                                                        let end = txt.encode_utf16().count() as u32;
                                                        ce_set_caret_utf16(&el, end);
                                                        editing_value.set(txt);
                                                        target_cursor_col.set(Some(end));
                                                    }

                                                    return;
                                                }

                                                if (key == "Backspace" || key == "Delete") && state == RoamDeleteState::Empty {
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

                                                // Shift+Enter: soft line break inside a node (do NOT create a new Nav).
                                                if key == "Enter" && ev.shift_key() {
                                                    if let Some(input_el) = input() {
                                                        // Prefer our DOM insertion to keep behavior consistent.
                                                        // But if it fails (e.g. transient selection issues right after typing),
                                                        // fall back to the browser default Shift+Enter behavior.
                                                        let is_empty = ce_text(&input_el).trim().is_empty()
                                                            && input_el.inner_html().trim().is_empty();

                                                        let ok = insert_soft_line_break_dom(&input_el);
                                                        let will_prevent = ok || is_empty;
                                                        if will_prevent {
                                                            ev.prevent_default();
                                                        }

                                                        if ok {
                                                            // Sync signal from DOM.
                                                            editing_value.set(ce_text(&input_el));
                                                        }
                                                    }
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
                                                            properties: None,
                                                        });
                                                    });

                                                    editing_id.set(Some(tmp_id.clone()));
                                                    editing_value.set(String::new());
                                                    editing_snapshot.set(Some((tmp_id.clone(), String::new())));
                                                    target_cursor_col.set(Some(0));

                                                    spawn_local(async move {
                                                        // If the current node is still a temporary (optimistic) node, do NOT
                                                        // send an update with a tmp id. The backend will reject it (400 Invalid nav ID).
                                                        // We'll backfill content once the real id is known.
                                                        if should_persist_nav_id(&nav_id_now) {
                                                            let _ = api_client.upsert_nav(save_req).await;
                                                        }

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
                                                                    editing_snapshot.set(Some((new_id.clone(), content_now.clone())));
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
                                        >
                                        </div>

                                        {move || {
                                            let popover_id = ac_popover_id_sv.get_value();
                                            let anchor_name = ac_anchor_name_sv.get_value();
                                            let open = ac_sv.get_value().ac_open.get();

                                            // A small JS bridge to sync `data-open` -> Popover API.
                                            let sync_script = format!(
                                                r#"(() => {{
  const pop = document.getElementById('{id}');
  if (!pop || pop.dataset.init) return;
  pop.dataset.init = '1';

  const sync = () => {{
    const open = pop.getAttribute('data-open') === 'true';
    try {{
      if (open) pop.showPopover();
      else pop.hidePopover();
    }} catch (_) {{}}
  }};

  const mo = new MutationObserver(sync);
  mo.observe(pop, {{ attributes: true, attributeFilter: ['data-open'] }});
  sync();
}})();"#,
                                                id = popover_id
                                            );

                                            view! {
                                                <>
                                                    <style>
                                                        {format!(
                                                            r#"
#{popover_id} {{
  position-anchor: {anchor_name};
  inset: auto;
  top: anchor(bottom);
  left: anchor(left);
  margin-top: 4px;
  @position-try(flip-block) {{
    bottom: anchor(top);
    top: auto;
    margin-bottom: 4px;
    margin-top: 0;
  }}
  position-try-fallbacks: flip-block;
  position-try-order: most-height;
  position-visibility: anchors-visible;
  z-index: 1000000;
}}
"#,
                                                            popover_id = popover_id,
                                                            anchor_name = anchor_name
                                                        )}
                                                    </style>

                                                    <div
                                                        id=popover_id
                                                        popover="manual"
                                                        data-open=open.to_string()
                                                        class="z-50 w-[28rem] max-w-[90vw] rounded-md border border-border-strong bg-background text-foreground p-1 text-sm shadow-lg"
                                                    >
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
                                                                <Command class="w-full" should_filter=false disable_scripts=true>
                                                                    <div class="max-h-64 overflow-auto" node_ref=ac_list_ref>
                                                                        <CommandList class="max-h-none min-h-0">
                                                                            {items
                                                                            .into_iter()
                                                                            .enumerate()
                                                                            .map(|(i, it)| {
                                                                                let title = it.title.clone();
                                                                                let title_for_insert = title.clone();
                                                                                let title_for_view = title.clone();
                                                                                let is_new = it.is_new;
                                                                                let selected = Signal::derive(move || i == idx);

                                                                                let ac = ac_sv.get_value();

                                                                                view! {
                                                                                    <CommandItem
                                                                                        value=title.clone()
                                                                                        selected=selected
                                                                                        class="flex items-center justify-between rounded px-2 py-1 hover:bg-surface-hover"
                                                                                        on_mousedown=Some(Callback::new(move |ev: web_sys::MouseEvent| {
                                                                                            // Prevent input blur.
                                                                                            ev.prevent_default();

                                                                                            if let Some(input_el) = editing_ref.get() {
                                                                                                let he: web_sys::HtmlElement = input_el.unchecked_into();
                                                                                                let v = ce_text(&he);
                                                                                                let (caret_utf16, _caret_end_utf16, _len) =
                                                                                                    ce_selection_utf16(&he);
                                                                                                let caret_byte = utf16_to_byte_idx(&v, caret_utf16);
                                                                                                let start_utf16 = ac.ac_start_utf16.get_untracked().unwrap_or(0);
                                                                                                let start_byte = utf16_to_byte_idx(&v, start_utf16);

                                                                                                let mut next = String::new();
                                                                                                next.push_str(&v[..start_byte.min(v.len())]);
                                                                                                next.push_str("[[");
                                                                                                next.push_str(&title_for_insert);
                                                                                                next.push_str("]]");
                                                                                                next.push_str(&v[caret_byte.min(v.len())..]);

                                                                                                ce_set_text(&he, &next);
                                                                                                editing_value.set(next.clone());

                                                                                                let caret_after = start_utf16
                                                                                                    + 2
                                                                                                    + (title_for_insert.encode_utf16().count() as u32)
                                                                                                    + 2;
                                                                                                ce_set_caret_utf16(&he, caret_after);
                                                                                            }

                                                                                            ac.ac_open.set(false);
                                                                                            ac.ac_start_utf16.set(None);
                                                                                        }))
                                                                                        on:mousemove=move |_ev| {
                                                                                            ac.ac_index.set(i);
                                                                                        }
                                                                                        attr:data-ac-idx=i.to_string()
                                                                                    >
                                                                                        <div class="truncate">{title_for_view.clone()}</div>
                                                                                        <Show when=move || is_new fallback=|| ().into_view()>
                                                                                            <div class="ml-2 shrink-0 text-xs text-muted-foreground">"Create"</div>
                                                                                        </Show>
                                                                                    </CommandItem>
                                                                                }
                                                                            })
                                                                            .collect_view()}
                                                                        </CommandList>
                                                                    </div>
                                                                </Command>
                                                            }
                                                            .into_any()
                                                        }}
                                                    </div>

                                                    <script>{sync_script}</script>
                                                </>
                                            }
                                            .into_any()
                                        }}
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

#[cfg(test)]
mod editor_delete_behavior_tests {
    use super::*;

    #[test]
    fn test_has_any_text_content() {
        assert!(!has_any_text_content(""));
        assert!(!has_any_text_content(" \n\t"));
        assert!(has_any_text_content("a"));
        assert!(has_any_text_content(" 爱 "));
    }

    #[test]
    fn test_effective_semantic_br_count() {
        assert_eq!(effective_semantic_br_count(0, false), 0);
        assert_eq!(effective_semantic_br_count(0, true), 0);
        assert_eq!(effective_semantic_br_count(1, false), 1);
        assert_eq!(effective_semantic_br_count(1, true), 0);
        assert_eq!(effective_semantic_br_count(2, true), 1);
    }

    #[test]
    fn test_roam_delete_state() {
        assert_eq!(roam_delete_state(true, 0), RoamDeleteState::HasContent);
        assert_eq!(roam_delete_state(true, 3), RoamDeleteState::HasContent);

        assert_eq!(roam_delete_state(false, 2), RoamDeleteState::OnlySoftBreaks);
        assert_eq!(roam_delete_state(false, 1), RoamDeleteState::OnlySoftBreaks);

        assert_eq!(roam_delete_state(false, 0), RoamDeleteState::Empty);
    }

    #[test]
    fn test_should_persist_nav_id() {
        assert!(!should_persist_nav_id(""));
        assert!(!should_persist_nav_id("   "));
        assert!(!should_persist_nav_id("tmp-1-2"));
        assert!(should_persist_nav_id(
            "00000000-0000-0000-0000-000000000000"
        ));
        assert!(should_persist_nav_id("abc"));
    }
}

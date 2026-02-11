# Phase 9 — Rich Text Editor (contenteditable) SPEC

Status: Draft (approved direction, pending a few clarifications)

## 0. Goals / Non-goals

### Goals
- Provide a true rich-text editing experience per outline node (Nav), similar to modern block editors.
- Use `contenteditable` for the editing surface.
- Preserve outliner semantics:
  - `Enter` creates a new block (new Nav)
  - `Tab` / `Shift+Tab` indent / outdent
  - Arrow navigation across visible nodes
  - Backspace/Delete on empty removes a node (soft-delete)
- Keep existing features working:
  - `[[...]]` link autocomplete
  - Wiki link hover preview
  - Backlinks
  - Drag & drop reorder (from bullet/triangle only)

### Non-goals (Phase 9 initial milestone)
- Collaboration / realtime multi-user editing
- Complex block types (tables, database views, callouts)
- Image upload pipeline (may support rendering by URL later)
- Full fidelity HTML/Markdown import of rich marks (start with plain text paste)

## 1. Data model

### 1.1 Storage location
Store structured rich-text under `Nav.properties` (assume JSON is preserved by backend):

```json
{
  "rt": {
    "version": 1,
    "doc": { /* rich text AST */ }
  }
}
```

### 1.2 Relationship with `Nav.content`
Use **dual representation** (recommended):
- `properties.rt.doc` is the source-of-truth rich-text document.
- `Nav.content` remains a **plain-text mirror** derived from the doc.

Rationale:
- Minimizes refactors: existing backlinks/link extraction can continue reading `Nav.content`.
- Allows incremental rollout: nodes without `properties.rt` still render/edit.

### 1.3 AST shape (v1)
A minimal AST that supports marks + links and is easy to serialize:

```json
{
  "type": "rt-doc",
  "content": [
    {
      "type": "paragraph",
      "content": [
        { "type": "text", "text": "Hello " },
        { "type": "text", "text": "world", "marks": ["bold"] },
        {
          "type": "link",
          "kind": "wiki",
          "ref": { "title": "Some Page" },
          "content": [{ "type": "text", "text": "Some Page" }]
        }
      ]
    }
  ]
}
```

Supported inline nodes (Phase 9 initial):
- `text` with optional `marks`: `bold`, `italic`, `code`
- `link`:
  - `kind: "wiki"` with `ref.title` for internal wiki links
  - future: `kind: "url"` with `ref.href` for external URLs

## 2. Rendering model

### 2.1 Read mode
- Render `rt.doc` to non-editable DOM.
- If a node has no `rt.doc`, render plain `Nav.content` as today.

### 2.2 Edit mode
- Render the same structure into a `contenteditable` surface.
- Do **not** rely on `document.execCommand` (deprecated / inconsistent).

## 3. Editing core (contenteditable)

### 3.1 Event strategy
Core events to handle:
- `beforeinput` (preferred): intercept insert/delete/paragraph operations
- `keydown`: enforce outliner-level behavior (`Enter`, `Tab`, navigation)
- `compositionstart` / `compositionend`: IME (Chinese/Japanese) stability
- `paste`: start with plain text paste

### 3.2 Selection mapping
Maintain a mapping between DOM selection and AST positions:
- Convert DOM selection -> (node path, offset) in AST
- After AST updates, restore DOM selection as close as possible

Acceptance requirements:
- IME text is not lost
- caret does not jump unexpectedly during normal typing

## 4. Outliner semantics

- `Enter`: commits current node and creates a new sibling node (existing behavior).
- Soft line break (intra-block newline):
  - TBD: if Roam supports it, implement `Shift+Enter` to insert a newline into the rich-text doc.
  - Implementation note: it should not create a new Nav.

- `Tab` / `Shift+Tab`: indent/outdent (existing behavior).
- `Backspace/Delete` on empty: soft-delete node (existing behavior).

## 5. `[[...]]` autocomplete and wiki links

### 5.1 Trigger
While editing, if caret is in a text node and the preceding text includes an unclosed `[[...` fragment, open autocomplete.

### 5.2 Insert behavior
On selection:
- Replace the `[[typed]]` fragment with a `link(kind=wiki, ref.title=...)` node.
- Update `Nav.content` mirror accordingly.

## 6. Migration / compatibility

- If `properties.rt` is missing:
  - On first edit: create a minimal doc `{paragraph:[text(content)]}`
- On save:
  - Persist `properties.rt.doc`
  - Persist plain-text mirror to `Nav.content`

## 7. Keyboard shortcuts (Phase 9 initial)

No toolbar required.
Implement:
- `Cmd/Ctrl+B`: toggle bold
- `Cmd/Ctrl+I`: toggle italic
- `` Cmd/Ctrl+` ``: toggle inline code

## 8. Definition of Done (Phase 9 initial milestone)

- contenteditable editor works with IME.
- Outliner core controls do not regress (Enter/Tab/Arrow/Backspace).
- `[[...]]` autocomplete works in rich-text editing.
- Wiki link hover preview continues to work.
- Backlinks continue to work (via `Nav.content` mirror).
- No teardown/disposed reactive panics during navigation/unmount.

## Open Questions

1) Soft line break: confirm Roam behavior.
   - If unknown, implement `Shift+Enter` as a feature gate (enabled by default once verified) so we can adjust without rewriting.

2) Link kinds and routing:
   - Internal wiki link is stored by `title` (stable) vs by `note_id` (stable, but requires resolution).
   - Proposed v1: store `title` and resolve to note_id at navigation time using existing note list.

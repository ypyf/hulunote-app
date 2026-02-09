# Hulunote-app User Manual (WIP)

This manual describes **current** user-facing behavior in hulunote-app.
It is intended to be clear, specific, and testable.

## 1. Concepts

- **Database**: a workspace.
- **Note**: a page within a database.
- **Outline node (Nav)**: a bullet/line inside a note.
  - Nodes form a tree via `parid` (parent id).
  - Sibling ordering is controlled by `same-deep-order`.
  - Collapse/expand is controlled by `is-display`.
  - Deletion is soft delete via `is-delete`.

## 2. Outline Editor (Roam-style)

The outline editor is available on the Note page.

### 2.1 Editing

- Click a node’s content to enter edit mode (inline).
- Leaving the input (blur) saves the node content.

### 2.2 Enter (create sibling)

When editing a node:

- `Enter` saves current node and creates a **next sibling** under the same parent.
- The new sibling is placed using **midpoint order**:
  - If there is a next sibling, new order = midpoint(current, next).
  - Otherwise, new order = current + 1.0.
- After creation, the editor moves into the new node.

### 2.3 Tab / Shift+Tab (indent / outdent)

When editing a node:

- `Tab` (indent):
  - The node becomes a child of the **previous sibling**.
  - The new parent is expanded.
  - The node is appended to the end of the new parent’s children.

- `Shift+Tab` (outdent):
  - The node becomes a sibling of its parent (i.e. parent’s parent becomes the new parent).
  - The node is placed immediately after its parent in sibling order.

### 2.4 Arrow keys (strict Roam navigation)

Navigation works inside the inline editor input.

#### Up / Down

- `ArrowUp` moves to the previous **visible** node.
- `ArrowDown` moves to the next **visible** node.
- Before moving, the current node content is saved (local state + backend upsert).
- The editor attempts to preserve the cursor column between lines.

“Visible” means preorder traversal of the outline tree, respecting collapse state:
- a node’s children are included only when `is-display` is true.

#### Left (line start)

At **line start** (cursor at column 0):

1) If a previous sibling exists, move to the previous sibling’s **last visible descendant** (or the sibling itself if collapsed/no children), and place the cursor at end.
2) If there is no previous sibling (i.e. first child), move to the **parent** and place the cursor at end.

Otherwise (cursor not at start), ArrowLeft behaves as normal text navigation.

#### Right (line end)

At **line end** (cursor at the end of the line):

- If the node has children:
  - If collapsed (`is-display=false`): expand AND descend into the first child.
  - If expanded: descend into the first child.
- If the node has no children: do **not** move to a sibling (strict Roam behavior).

Otherwise (cursor not at end), ArrowRight behaves as normal text navigation.

### 2.5 Delete empty node (soft delete)

When editing a node:

- `Backspace` / `Delete` on an **empty** node soft-deletes it:
  - The node and its subtree are removed from local state.
  - Backend is updated using `is-delete: true`.
  - Focus moves to the previous visible node if possible, otherwise next visible.

## 3. Known limitations (current)

- Drag-and-drop reordering is not implemented.
- Multi-line nodes (textarea) / autosizing is not implemented.
- Full Roam block operations (merge/split, block references) are out of scope.

---

Last updated: 2026-02-10

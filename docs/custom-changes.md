# Custom (non-upstream) changes

Our fork's changes are tagged in code with `RP-CUSTOM[<id>]`. Before/after an upstream
sync, run `git grep "RP-CUSTOM"` to find every spot to preserve or re-apply.
Block edits use `BEGIN`/`END` markers; single-line edits carry the tag in an adjacent comment.

## hover-checkbox — Google Photos-style multi-select

Hover a thumbnail to reveal a top-left checkbox; clicking it enters select mode.
Right-click behaviour in select mode is unchanged upstream behaviour (acts on the selection
if the photo is in it, otherwise selects just that photo).

| File | What differs from upstream |
|------|----------------------------|
| `src-vite/src/components/Thumbnail.vue` | Checkbox moved top-right → top-left, rendered outside select mode too (visible on hover); status-badge container shifts right in select mode. |
| `src-vite/src/components/Content.vue` | `handleItemSelectToggled`: block at the top that enters select mode when the event arrives outside it (Shift extends from the focused item). |

Upstream sync tip: if upstream rewrites the thumbnail checkbox, keep our template block and
the `handleItemSelectToggled` guard; both are self-contained.

## multi-select-menu — right-click menu rules for selections

| File | What differs from upstream |
|------|----------------------------|
| `src-vite/src/common/fileMenu.ts` | In `buildSelectionMenu`: **Send to Smart Photos** requires an all-image selection (`kind !== 'image'` disables); **Create montage** is disabled for video-only selections. |
| `src-vite/src/components/Content.vue` | `singleSelectedFile` / `singleSelectionMenuItems` block (BEGIN/END) plus a small switch in `handleSelectionContextMenu`: with exactly one photo ticked, right-click shows the full single-photo menu. The selection panel still uses `selectionMenuItems`. |

# Smart Photos: Design Document

Status: Implemented (not yet tried in the running app)
Date: 2026-11-23

## 1. Goal

Let the user select one or more photos or folders in the Lap main screen, right-click, and choose **Send to Smart Photos**. The app **copies** the files into a separate, curated "Smart Photos" folder tree. The tree is organized by year and then by event folder. The source library is never modified.

Example destination: `F:\FavMoments\<year>\<event folder>`, e.g. `F:\FavMoments\2010\2010_01_01`.

## 2. Non-goals

- Moving or deleting source files. See the read-only rule in section 4.
- Copying RAW files or videos.
- Preserving NTFS alternate data streams or other NTFS-specific extras.

## 3. Requirements

| # | Requirement |
|---|-------------|
| R1 | Right-click on a selection in the main grid shows **Send to Smart Photos**. |
| R2 | The Smart Photos root folder is set in Settings and saved for future sessions. |
| R3 | Destination is `<root>\<year>\<event folder>`. Missing year and event folders are created before copying. |
| R4 | Only `.jpg`, `.jpeg` and `.png` files are processed. Other types are ignored silently. |
| R5 | A selected folder is expanded to the supported files it contains. RAW files and videos inside it are ignored. |
| R6 | Copying preserves EXIF and file properties (see section 7). |
| R7 | A name collision is skipped when size and modified time match, and overwritten otherwise. |
| R8 | The result is shown as a toast, e.g. "3 copied, 1 skipped (already exists)". |
| R9 | Photos that are already in Smart Photos are marked in the UI. |

## 4. Source is read-only (critical)

**The source is strictly read-only.** This rule has priority over every other requirement.

- Open source files for reading only (`File::open`, never with write or truncate access).
- Never rename, move, delete or touch the timestamps of a source file or folder.
- Only the destination copy gets its timestamps set. Never apply timestamp changes to the source path.
- The destination path must never resolve inside the source library. Reject the operation if it does, for example when the Smart Photos root is misconfigured to point at the library.
- Overwrites (R7) apply only to files under the Smart Photos root.
- The copy command should not be given any code path that takes the source path as a write target. Keep source and destination as separate typed parameters.

## 5. Destination path rules

For each source file `<...>\<parent folder>\photo.jpg`:

1. **Event folder** = name of the file's immediate parent folder.
2. **Year** = name of the parent of that folder, if it is a four-digit year.
3. Destination = `<smart_photos_root>\<year>\<event folder>\photo.jpg`.

Create `<year>` and `<event folder>` if they do not exist (`create_dir_all`).

Code comment to place on the year-resolution function:

```rust
// ASSUMPTION: the source library is laid out as <year>\<event folder>\photo.jpg.
// The year is therefore taken from the name of the event folder's parent folder.
// If the layout differs, the caller must ask the user before falling back to the
// photo's EXIF date or the file's modified date.
```

### 5.1 Fallback when the layout does not match

If the parent-of-parent folder is not a four-digit year:

1. Do not copy anything yet. Show an alert: "Some selected photos are not in a `<year>\<event folder>` layout. Use the photo's date instead?" It includes the count of affected files.
2. If the user declines, those files are skipped (counted as "skipped, layout" in the toast). Files with a valid layout are still processed.
3. If the user agrees, resolve the year for those files in this order:
   1. EXIF `DateTimeOriginal`, then `CreateDate`.
   2. File modified date.
4. The event folder is still the immediate parent folder name.

The alert is shown once per send operation, not once per file.

## 6. File selection

- Allowed extensions, case-insensitive: `jpg`, `jpeg`, `png`.
- For a selected folder, walk it recursively and collect only allowed files. RAW files, videos and everything else are ignored without any message.
- Each file keeps its own parent folder as its event folder, so subfolders inside a selected folder map to their own event folders.
- Selected files and folders are combined and de-duplicated by path before processing.

## 7. Copy behaviour

For each file:

1. Compute the destination path (section 5).
2. If the destination exists and has the same **size** and **modified time** as the source, **skip** it (count as "already exists").
3. Otherwise copy byte for byte (`std::fs::copy`) and **overwrite** any existing file. A byte-for-byte copy keeps EXIF and all embedded metadata intact.
4. After the copy, set the destination's **created**, **modified** and **accessed** times to the source's values. On Windows this uses `SetFileTime`, or a crate such as `filetime` plus a Windows API call for the creation time.
5. Out of scope: NTFS alternate data streams, ACLs and other NTFS extras.

The modified-time comparison should use a small tolerance (for example 2 seconds). This avoids false mismatches on file systems with coarser timestamps, such as exFAT.

The work runs on a background task so the UI stays responsive. Errors are recorded per file. One failed file does not stop the others.

## 8. Settings

- New setting: **Smart Photos folder**, stored in the app's existing settings storage.
- One global Smart Photos path (confirmed). It is not per library.
- Folder picker in Settings (confirmed).
- Drive warning (confirmed): when the saved folder is not reachable, for example because the drive `F:` is not connected, show a warning next to the setting in Settings. Re-check each time the Settings page opens.
- **Send to Smart Photos** is disabled, or prompts the user to open Settings, when no folder is configured.
- Validate on send: the folder exists and is writable. If the drive is not connected, show the same warning as an error and do nothing.

## 9. UI

- **Context menu:** add **Send to Smart Photos** to the main-grid right-click menu (see `src-vite/src/common/fileMenu.ts`). It applies to the full multi-selection, including folders.
- **Viewer (confirmed):** the viewer's right-click menu also has **Send to Smart Photos**, for the file being shown.
- **Folders:** the sidebar folder context menu has **Send to Smart Photos** for a whole folder (the main grid lists files only).
- **Menu label (confirmed):** "Send to Smart Photos". Add the string to both `en.json` and `nl.json`, so the two locale files stay in step. The Dutch text will be a translation of the English label, and you can adjust it.
- **Toast** after each send:
  - Normal: "3 copied, 1 skipped (already exists)".
  - Include other counts only when non-zero, e.g. "2 skipped (layout)", "1 failed".
  - Errors list the first few failure reasons, with the rest available in the log.

### 9.1 Marking photos already in Smart Photos (R9)

A photo counts as "in Smart Photos" when the file at its computed destination path exists and has the same size and modified time as the source. This is the same check as the skip rule in section 7.

- Show a small badge on the thumbnail.
- Compute lazily for the visible items only, on a background task, and cache the result. Do not hit the destination drive for the whole library at load time.
- Invalidate the cache for a file after a send, and when the Smart Photos root setting changes.
- If the destination drive is offline, show no badges rather than an error.

Open point: whether the badge also appears in the viewer and in the file info panel. The default is the grid only.

## 10. Implementation outline

Backend (`src-tauri`):

- `smart_photos` module with:
  - `resolve_destination(source, root, allow_date_fallback) -> Result<Dest>`
  - `collect_files(paths) -> Vec<PathBuf>` (extension filter, folder walk)
  - `send_to_smart_photos(paths, allow_date_fallback) -> SendResult { copied, skipped_exists, skipped_layout, failed }`
  - `check_in_smart_photos(paths) -> Vec<bool>` for the badges
- Tauri commands wrapping those functions.
- Settings field for the Smart Photos root.

Frontend (`src-vite`):

- Menu entry, confirmation dialog for the layout fallback, toast, and thumbnail badge.
- Settings page entry with folder picker.

The command takes the paths of the selected files and folders. It resolves the layout and counts the files that need the fallback. If that count is above zero, it returns it to the UI so the dialog can ask. The UI then calls the command again with `allow_date_fallback` set to the user's choice.

## 11. Testing

- Year taken from the parent folder; destination folders created when missing.
- Layout mismatch: the alert appears, "No" skips the files, and "Yes" uses the EXIF date and then the modified date.
- Unsupported types (RAW, video) inside a selected folder are ignored.
- Existing file with matching size and mtime is skipped. A different size or mtime is overwritten.
- The copy is byte-identical (hash match) and the created, modified and accessed times match the source.
- **The source file's bytes and timestamps are unchanged after a send, including after an overwrite and after an error.**
- Destination inside the source library is rejected.
- Drive offline and root unset are handled without a crash.
- Badge shows correctly after a send, and clears when the root changes.

## 12. Open questions

- Should the badge also appear in the viewer and in the file info panel?
- A send made from a separate viewer window does not refresh the main window's badges until the cache is cleared (a send from the main window does).

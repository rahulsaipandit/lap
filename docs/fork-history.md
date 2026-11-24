# Fork history (custom changes vs. upstream julyx10/lap)

Every change below is ours, not upstream's. Each is marked in code with a comment starting
`RP-CUSTOM` so it survives an upstream sync. Before/after a sync run:

```
git grep -n "RP-CUSTOM"
```

Newest first. Dates come from `git log` (author rahulsaipandit); uncommitted work uses the date
in its design doc or "uncommitted".

| Date | Change | Why (2 lines) | Code markers |
|------|--------|---------------|--------------|
| 2026-11-23 (uncommitted) | **Multi-select right-click menu rules** | A single ticked photo (e.g. after the hover checkbox) now gets the full single-photo menu.<br>Montage is disabled for video-only selections and Send to Smart Photos needs an all-image selection. | `Content.vue`, `fileMenu.ts` (`multi-select-menu`) |
| 2026-11-23 (uncommitted) | **Send to Smart Photos** (right-click copy into `<root>\<year>\<event>` tree; Settings root path) | Curate favourite photos into a separate year/event folder tree without touching the source library.<br>Copy-only by design; sources are never modified. See `docs/smart-photos-design.md`. | `t_smart_photos.rs`, `smartPhotos.ts` |
| uncommitted | **Blur finder improvements** (BlurryPane scope/scan rework, scan state) | Make the blurry-photo pane usable on large libraries and resumable.<br>Builds on the 2026-11-18 blur feature. | `t_blur.rs`, `BlurryPane.vue`, `Content.vue` |
| uncommitted | **Hover checkbox multi-select** (Google Photos style) | Hovering a thumbnail shows a top-left checkbox that enters select mode.<br>Faster multi-select than the upstream select-mode toggle. Details in `docs/custom-changes.md`. | `Thumbnail.vue`, `Content.vue` (`handleItemSelectToggled`) |
| 2026-11-21 | Rotate-left + save rotation to file: UI polish; GetStarted guide refresh | Round out the rotation feature (left rotate, keep file modified date when writing EXIF).<br>Guide updated so new users can build and run the fork. | `t_cmds.rs`, `t_image.rs`, `rotation.ts` |
| 2026-11-20 | **Rotate-left button and "save rotation to file"** (EXIF Orientation, JPEG, lossless) | Upstream rotation only lived in the app DB, so Explorer and other apps showed the old orientation.<br>Writing the EXIF tag makes the rotation real without re-encoding pixels. | `t_cmds.rs` (`can_apply_file_rotation`), `t_image.rs` (`rotation_write_supported`), `rotation.ts` |
| 2026-11-19 | **Fix stale dedup results for deleted files; dedup pane scans on first load** | Files deleted outside Lap still appeared in duplicate groups, and the pane showed nothing until a manual rescan.<br>Now deleted files and their cached hashes are dropped, and the pane scans on open. | `t_dedup.rs`, `DedupPane.vue` |
| 2026-11-18 | Fix tests after migration 25; add test/run instructions | The blur-scan migration bumped `user_version` to 25, breaking migration tests that asserted 24.<br>Docs gained instructions on running them. | `t_migration.rs` |
| 2026-11-18 | **Stale-hash detection in dedup compares `file_size` as well as mtime** | A file edited in place with an unchanged mtime kept its old hash and was grouped wrongly.<br>Comparing size too catches these edits and forces a re-hash. | `t_dedup.rs` (`check_if_needs_hash`) |
| 2026-11-18 | Design/analysis docs in `docs/guide/` (design, indexing analysis); cleaned to only real code bugs | Capture how dedup/indexing works and which behaviours are real bugs versus documentation gaps.<br>Speculative issues were removed to keep the docs trustworthy. | docs only |
| 2026-11-18 | Merge of upstream/main (v0.4.0 era) | Routine upstream sync. Re-check every `RP-CUSTOM` marker after merges like this. | n/a |
| 2026-11-18 | **WIP: PromptFix AI fix, blur detection, face/cluster changes** | Find blurry photos (scan tables, migration 25) and offer an AI "fix" via the local PromptFix service.<br>Face clustering was reworked alongside; see `docs/guide/designDeBlur.md`, `Person_FaceRecognition.md`. | `t_blur.rs`, `t_migration.rs`, `t_cluster.rs`, `BlurryPane.vue`, `AiFixDialog.vue`, `promptfix_service/app.py` |
| 2026-08-07 | Fix `t_heif.rs` extern declarations left commented out after merge | `heif_image_handle_get_width/height` are called by merged code but their `extern "C"` declarations were commented out.<br>Re-enabled so the build links. | `t_heif.rs` |
| 2026-08-07 | Sync from main to fork branch (dedup docs) | Brought `DuplicateDetectionFeature.md` and `WinMergeRequirements.md` into the fork branch. | docs only |
| 2026-05-05 | **Separate, more visual duplicate detection pane (`DedupPane_2`)** | A WinMerge-style visual duplicate review separate from the built-in dedup.<br>Since superseded by upstream's Dedup pane (`DedupPane.vue`); `DedupPane_2.vue` no longer exists. Also touched macOS-only video/HEIF `use` tweaks. | none remaining (history only) |
| 2026-05-04 | README and `package.json` updates | Document the fork and its build steps. | docs only |

## Conventions

- Marker prefix: `RP-CUSTOM` (e.g. `// RP-CUSTOM: <one-line reason>`; Vue templates use
  `<!-- RP-CUSTOM: ... -->`). Block edits may use `BEGIN`/`END`.
- Add a row here whenever a custom change is made. See `CLAUDE.md`.

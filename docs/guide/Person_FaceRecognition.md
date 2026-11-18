# Person & Face Recognition

Lap detects faces, groups them into people, and lets you browse photos by person. Everything runs locally on-device using ONNX models; no photos leave the machine.

> Status of this document: verified against the code in `src-tauri/src` and `src-vite/src`. Items marked **Known limitation** are real behaviours of the current implementation, not planned features.

## Features

- **Face detection and embedding** — faces are detected, embedded and clustered into people automatically.
- **Person View** — a grid of people; selecting one lists that person's photos (filter by `person_id`).
- **Rename / delete a person** — `rename_person`, `delete_person`.
- **Viewer overlay** — detected face boxes are shown in the image viewer (`get_faces_for_file`).
- **Cluster strictness setting** — Settings → face cluster threshold (see [Clustering](#clustering)).

Not present in this codebase (despite what older release notes imply):

- **Search by typed person name.** No query matches `persons.name`; people are chosen from the Person View by `person_id`. Names are only for display and sorting.
- **Merge clusters / move a face to another person.** No backend command exists for this. `Face::assign_to_person` is used only by the clusterer. The v0.1.1 release notes mention face merging; if the UI offers it, it is not backed by a command in this tree. Confirm before documenting it to end users.

## Code map

| Area | File |
|---|---|
| Detection, embedding, indexing loop | [t_face.rs](../../src-tauri/src/t_face.rs) |
| Clustering | [t_cluster.rs](../../src-tauri/src/t_cluster.rs) |
| Constants and model file names | [t_common.rs](../../src-tauri/src/t_common.rs) |
| Commands | [t_cmds.rs](../../src-tauri/src/t_cmds.rs): `index_faces`, `get_face_stats`, `cancel_face_index`, `reset_faces`, `is_face_indexing`, `get_persons`, `get_persons_page`, `rename_person`, `delete_person`, `get_faces_for_file` |
| Schema and queries | [t_sqlite.rs](../../src-tauri/src/t_sqlite.rs) (`Face`, `Person`) |
| UI | `src-vite/src/components/Person.vue`, `src-vite/src/views/Settings.vue`, `src-vite/src/common/api.js` |

## Models

All models are ONNX and run through ONNX Runtime.

| Purpose | Model file | Notes |
|---|---|---|
| Face detection | `det_500m.onnx` | SCRFD-style detector (the comment in code says RetinaFace; the decoding code assumes SCRFD `det_10g`-style outputs) |
| Face embedding | `w600k_mbf.onnx` | MobileFaceNet, 512-dim, L2-normalized |
| Image search (vision) | `vision_model.onnx` | Whole-image embedding |
| Image search (text) | `text_model.onnx` | Optional multilingual version downloaded on demand (see [t_ai.rs](../../src-tauri/src/t_ai.rs)) |

The image-search pair looks CLIP-style; the exact variant is not named in the code.

## Storage

Your folder layout (for example `Year/YYYY_MM_DD`) has no effect on how embeddings are stored. Everything is in one SQLite database, keyed by file ID, not by folder path. A person can appear in any year or date folder.

### Photo embedding (whole-image search)

- `afiles.embeds` (BLOB): one vector per file, used for natural-language and similar-image search.
- Query-time cosine similarity against every stored vector (brute force).
- Unrelated to faces; face data is never mixed into this column.

### Face / person recognition

```
afiles (id, folder_id, has_faces, embeds, ...)
   ▲
   │ faces.file_id  (ON DELETE CASCADE)
faces (id, file_id, bbox JSON, embedding BLOB, person_id, ignored, created_at)
   │
   │ faces.person_id  (ON DELETE SET NULL)
   ▼
persons (id, name, cover_face_id, thumbnail BLOB, locked, created_at)
```

- `faces`: one row per kept face. `embedding` is 512 little-endian float32 values. `bbox` is JSON `{x, y, width, height, confidence}` in **original-image pixels**.
- `persons`: `name` is nullable and not unique. The cover face is the largest face (by bbox area) of that person; `thumbnail` is a cached crop.
- `persons.locked`: `1` once the user has renamed the person. Locked persons are anchors that re-indexing never re-clusters (see [Clustering](#clustering)). Existing databases are migrated once: any person whose name is not an auto-generated `Person N` becomes locked.
- `faces.ignored`: set to `1` when the person owning the face is deleted. Ignored faces are skipped by clustering, so a deleted person does not come back. `has_faces` is untouched, so the files are not rescanned. `reset_faces` clears everything.
- `afiles.has_faces` is a scan status, not a boolean: `NULL`/`0` = not scanned, `1` = scanned and faces stored, `2` = scanned and no faces kept.
- Indexes exist on `faces.file_id` and `faces.person_id`.
- Only photos are scanned (`file_type = 1` with known width and height). Videos are excluded.

## Indexing pipeline

Triggered by `index_faces` (Person View → "Index faces"), from [t_face.rs](../../src-tauri/src/t_face.rs):

1. Load the two face models if not already loaded. Only one indexing run may be active at a time.
2. Select unscanned photos (`has_faces IS NULL OR 0`). Files already marked `1` or `2` are skipped, so runs are incremental.
3. For each file, run detection on the **cached thumbnail** when one exists (falls back to the original on error). Bboxes are then scaled back up to original dimensions.
4. Filter each detection:
   - confidence `< 0.65` → dropped (`MIN_CONFIDENCE`)
   - Laplacian-variance blur score of the face crop `< 200` → dropped (`MIN_BLUR_SCORE`)
   - minimum face size / area filters exist in code but are commented out
5. Embed surviving faces, insert into `faces`, set `has_faces` to `1` (or `2` if none survived). Progress is emitted every 10 files; cancellation is checked between files.
6. After the loop (if not cancelled), re-cluster **all** faces (below).

`reset_faces` clears faces so the library can be rescanned; `cancel_face_index` stops a run.

## Clustering

[t_cluster.rs](../../src-tauri/src/t_cluster.rs) runs after every indexing pass. It keeps people the user has named fixed, attaches new faces to them, and clusters only the rest.

1. **Load** every non-ignored face and split it:
   - **anchors**: faces of **locked** persons (renamed by the user). They stay exactly where they are.
   - **free faces**: everything else (unassigned, or in an automatic `Person N`).
2. **Attach free faces to anchors.** A free face joins the locked person whose nearest reference embedding is within the threshold (cosine distance `< threshold`). At most 200 evenly sampled reference embeddings are compared per locked person. A face never joins a person who already has a face in the **same photo**, and when a photo has several candidates the closest face is placed first.
3. **Cluster the remaining free faces** with **Chinese Whispers** over a thresholded K-NN graph: compare every pair (O(n²) over the remaining faces only), edge when distance `< threshold` with weight `(1 − distance)²`, never between faces of the same photo, top `K = 80` edges per node, label voting in random order for up to 20 iterations. Keep clusters with at least `MIN_SAMPLES = 1` faces.
4. **Write the result** with `Person::apply_clusters` ([t_sqlite.rs](../../src-tauri/src/t_sqlite.rs)) in a single transaction, then regenerate thumbnails.

Nothing is written before step 4, so cancelling or failing mid-run leaves the previous result intact.

### Preserving names across re-indexing

- **Renaming locks a person.** `rename_person` sets `persons.locked = 1`. A locked person keeps its row (`id`, `name`, `cover_face_id`) and all its faces, regardless of threshold changes or new photos, and gains new faces that are similar enough.
- **Automatic persons stay stable.** Each new cluster is matched to the previous *automatic* person it overlaps most (greedy by overlap, one-to-one, deterministic ties). A match requires the overlap to cover at least half of the smaller side, so an id cannot jump onto an unrelated cluster.
- Unmatched clusters become new persons, largest first, named `Person N` where `N` is above every surviving `Person N` (locked persons included), so default names never collide.
- Persons left with no faces are deleted.
- **Deleting a person is permanent.** `delete_person` marks the person's faces `ignored`, so they are not re-clustered into a new person.

Threshold: the UI offers `[0.35, 0.45, 0.55, 0.65]` (config `faceClusterThresholds`; fallback `0.55` in `Person.vue`). The Rust fallback if none is passed is `0.42`. Lower is stricter; higher merges more. A new threshold affects only free faces and which new faces attach to locked persons; faces already in a locked person never move.

Because label voting uses a random order, the clusters of **automatic** persons are not deterministic between runs. Locked persons are.

## Design review

Ordered by impact.

### Fixed

1. **Every indexing run erased user names and manual edits.** Previously `cluster_faces` nulled all `person_id` and ran `DELETE FROM persons` before re-clustering, so each re-index discarded renames, cover faces and person ids; a cancelled run left the library with no persons at all. Now renamed persons are locked anchors, automatic persons keep their ids by overlap matching, and nothing is written until the final transaction.
2. **A deleted person came back on the next index.** Deleted persons' faces are now marked `ignored`.
3. **A named person could split or lose its name after a threshold change.** Locked persons are never re-clustered; new faces attach to the nearest one.

Remaining limitations of this design:

- There is **no un-lock and no un-ignore**. Renaming is a one-way lock, and a deleted person can only come back via `reset_faces`. If a locked person was built from a wrong cluster, nothing can fix it until merge/split exist (item 2).
- A locked person can only be extended by faces within the threshold of its sampled references. A person who looks very different across years may still be split into a new automatic `Person N`; renaming that one to the same name creates two people with the same name (names are not unique).
- Free faces that were wrongly attached to a locked person cannot be detached.

### Critical

None open.

### High

2. **No way to correct mistakes.** There is no merge, split, or move-face-to-person command, and no "not this person" action. With a probabilistic clusterer and a coarse 4-step threshold, one person commonly ends up in several clusters. Renaming is the only repair, and since renaming now locks the person, a wrong cluster that was named cannot be repaired either.
3. **Singleton clusters become people.** `MIN_SAMPLES = 1` creates a person for every unmatched face, so the Person View fills with one-photo "people" (strangers, background faces). Raise the minimum (e.g. 3) or put small clusters into an "Unnamed / Other" group.
4. **O(n²) clustering, fully in memory, on every run.** Locked persons no longer take part in the pairwise step, which helps once people are named, but unnamed faces are still compared pairwise: at 200k free faces that is about 2×10¹⁰ comparisons, single-threaded and not resumable. Use an ANN index (HNSW) or blocking, run it in parallel, and only cluster the new faces against existing automatic persons.
5. **Name search is missing.** The feature as a user would expect it ("find photos of Alice") works only by clicking a person tile. Add a text search that matches `persons.name` and resolves to `person_id`.

### Medium

6. **Detection runs on thumbnails, and blur is measured on the thumbnail crop.** Small or distant faces have few pixels in a thumbnail, so the Laplacian-variance score (threshold 200, tuned for full-resolution images) is biased low and valid faces are rejected. The file is then marked `has_faces = 2` and is never retried, even if the threshold or model changes. Consider scoring blur on the original, or storing the reason for rejection and offering "rescan".
7. **Quality filtering is coupled to scan status.** Because rejected faces are not stored, changing `MIN_CONFIDENCE`/`MIN_BLUR_SCORE` requires a full `reset_faces` and rescan. Storing all detections with a quality score and filtering at clustering time would avoid this.
8. **Non-deterministic clusters and an inconsistent default threshold** (UI `0.55` vs Rust `0.42`). Seed the shuffle, or iterate in a fixed order, and keep one source of truth for the default.
9. **Embedding alignment.** I did not verify whether faces are landmark-aligned before MobileFaceNet; unaligned crops noticeably reduce recognition accuracy. Check `get_face_embedding` in [t_face.rs](../../src-tauri/src/t_face.rs).
10. **Stale data on file change.** I did not verify that `has_faces`, `faces`, and `afiles.embeds` are reset when a file is edited, replaced, or rotated, or when `bbox` coordinates depend on EXIF orientation. Confirm this in the file-update paths.

### Low

11. `persons.name` is nullable and non-unique; two people can share a name and nothing prevents it. Default names (`Person N`) are sorted with a natural-number comparator, which is correct but only useful while the names stay generated.
12. Face embeddings are sensitive biometric data. They are stored unencrypted in the library database. This is local-only, but worth stating in the privacy docs, along with how `reset_faces` and removing a file affect them (`ON DELETE CASCADE` handles file removal).
13. The photo embedding search is brute force over every row and does not scale beyond hundreds of thousands of images without an index.

## Open questions to confirm before publishing user-facing docs

- Does the UI expose merge or move-face actions? If yes, find the backing code; if no, correct the v0.1.1 release note.
- Is there a name-based search anywhere in the frontend, beyond the Person View tile list?
- Alignment, orientation, and stale-data behaviour (items 9 and 10).

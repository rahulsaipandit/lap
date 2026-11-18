# Lap Design Document: Issues, Clarifications & Reality Check

This document addresses gaps between the design document (`design.md`) and the actual implementation.
Every claim below was checked against the source after merging `julyx10/lap` upstream (2026-11-18):
`src-tauri/src/t_dedup.rs`, `t_similar.rs`, `t_sqlite.rs`, `t_migration.rs`, `t_utils.rs`, and the frontend config in `src-vite/src/stores/configStore.js`.

Each issue lists **Status** (what is still open in code vs. fixed in docs) and the **Doc fix** applied to `design.md`.

---

## Issue 1: Hashing Every File Reads All Data (Hours on 1TB)

### Correction to the original analysis
The claim that every file is fully read on first run is wrong. Two filters run before any hashing:

1. `get_files_by_sizes` (SQL, `HAVING COUNT(size) > 1 AND size > 0`) and `filter_suspicious_files` keep only files whose `size` is shared by at least one other file. Files with a unique size cannot be exact duplicates and are never read.
2. `check_if_needs_hash` skips files whose stored `mtime` equals the current `modified_at`.

Hashing (`compute_blake3_hash`) streams the file through a 64 KB buffer, so memory use is flat.

### Real cost
- Mixed photo/video libraries: only same-size candidates are read, usually a small fraction of the library.
- Worst case (many same-size files, e.g. fixed-size RAW/exports): approaches a full read. 1 TB at 100-150 MB/s (HDD) is about 2-3 hours; at ~500 MB/s (SSD) about 35 min. BLAKE3 is not the bottleneck, I/O is. (Estimates from throughput, not benchmarks.)
- Re-scan: unchanged files are skipped on `mtime`; cost is DB queries only.

### Resource contention
Dedup runs on its own `std::thread::spawn` thread. Thumbnails and embeddings are throttled by the `tokio` semaphores in `ProcessingBudget` (`normal_thumb`, `heavy_thumb`, `embedding` in `t_utils.rs`). Dedup hashing is **not** under those semaphores, so it can compete with thumbnail generation for disk bandwidth.

### Status
- Doc: **fixed** (pre-filter, mtime skip, worst-case range).
- **Still open (code, all optional):**
  - Prefix pre-hash: hash a 64 KB head+tail first and full-hash only on collision.
  - Gate dedup hashing behind a low-priority semaphore, or pause while thumbnail/embedding queues are busy.
  - Surface "N candidates of M files" in scan progress.
  - **Known gap:** `check_if_needs_hash` compares only `mtime`. A file replaced with the same mtime keeps a stale hash. `file_size` is already stored in `file_hashes` and should also be compared.

---

## Issue 2: Near-Duplicates Not Covered by Hash

### What BLAKE3 detects
Exact byte-for-byte copies only. Re-compressed, resized, rotated or edited copies are not found.

### What Lap does (verified in `t_similar.rs`)
CLIP image embeddings plus an in-memory HNSW index (cosine distance). Actual constants:

```
MAX_GROUP_SIZE = 64
TOP_K          = MAX_GROUP_SIZE - 1 = 63
SEARCH_EF      = 200
HNSW           = Hnsw::<f32, DistCosine>::new(16 /*M*/, n /*max elements*/, 16 /*max layers*/, 200 /*ef_construction*/, DistCosine)
```

**Threshold is now a parameter, not a constant.** The old `MIN_SCORE = 0.93` was removed. `similar_start_scan` receives `similarity_threshold` (clamped 0-1) from the UI. The frontend offers four presets, `similarPhotoGroupingThresholds = [0.97, 0.93, 0.90, 0.85]` in `configStore.js`, with default index 1 (0.93). This covers the earlier suggestion to expose it as a setting.

Clustering: neighbour pairs with `score >= similarity_threshold` become edges, sorted by score descending. Two clusters merge only if **every** cross pair is an edge at or above the threshold (complete linkage) and the merged size stays <= 64. A weak A-B-C chain cannot form a group. This is complete-linkage clustering, not transitive closure (which would chain).

Results are stored in `similarity_scans` / `similarity_groups` / `similarity_group_items` (created in migration 10), scoped by `scope_key`. Group items carry `score` and `is_keep`; keep state is persisted (migration 15).

Limitation: HNSW returns approximate neighbours, so recall is not 100%, and only the top 63 neighbours per image are considered.

### Status
- Doc: **fixed** in `design.md` (Similarity Search section now describes CLIP + cosine + HNSW + complete linkage; the "Euclidean distance" and any perceptual-hash wording are removed; no perceptual hash exists in the code).
- Code: nothing open.

---

## Issue 3: Brute-Force Vector Search

### Corrections
- There is **no `ai_embeddings` table**. Embeddings are a little-endian `f32` BLOB in `afiles.embeds` (`update_embedding` / `get_embedding_by_id` in `t_sqlite.rs`).
- Text and image-by-example search (`AFile::search_similar_images`, `t_sqlite.rs` ~line 7440) is **brute force**: it selects `id, embeds` for every file with an embedding (applying search-exclusion, album and file-type filters), computes cosine similarity in Rust (`cosine_similarity_blob`), keeps scores **strictly above** `params.threshold`, sorts all survivors, then fetches the file rows. It is not SQL `ORDER BY euclidean_distance`.
- **Thresholds are now user/UI driven**, not a fixed 0.25:
  - Text search: `imageSearchThresholds = [0.32, 0.29, 0.26, 0.255]` (Strict/Focused/Standard/Broad), default Standard (0.26).
  - "Find related photos" (image-to-image): fixed 0.7.
  - Smart tags: fixed 0.25.
- Cost: O(N) per query, reading N x 512 x 4 B (2 KB per image). 100k images is about 200 MB read and scored per search; 1M is about 2 GB, too slow for interactive use.
- Only visual-similarity grouping (`t_similar.rs`) uses HNSW, built in memory per scan and not persisted.

### Status
- Doc: **fixed** (embedding storage, linear cosine scan, thresholds, HNSW scope).
- **Still open (code, recommended):**
  - Cache normalised embeddings in memory (invalidate on embedding update) and precompute norms, so each search avoids re-reading blobs from SQLite.
  - Use a bounded min-heap of size `limit` instead of collecting and sorting all scores. (Today there is no `limit` truncation before the sort; all results above threshold are sorted and fetched.)
  - Later: reuse an HNSW index for search once libraries exceed about 100k embeddings, persisted between runs.

---

## Issue 4: Performance Figures at 10k Files Only

### Benchmarks from the design doc
| Operation                 | Claimed time (10k files) |
|---------------------------|--------------------------|
| Initial index             | 30-40 s                  |
| Re-scan (unchanged)       | 5-10 s                   |
| Thumbnail generation (4x) | 20-30 s                  |

### Reality
The figures were measured only at 10k files. Larger numbers in earlier versions were linear extrapolations and must not be presented as benchmarks. Expected non-linear effects: SQLite index growth, brute-force search (Issue 4), and the O(n x k) similarity pass. Dedup cost depends on the same-size candidate set (Issue 2), not library size.

### Status
- Doc: **fixed**: `design.md` labels the table "measured at 10k files" and notes that larger sizes are unmeasured; hash cost is described as O(same-size candidates), not O(library).
- **Still open (action):** generate a synthetic 100k-file library and record indexing, re-scan, text-search latency and similarity scan time with the machine spec (CPU, disk type). Add HDD / SSD / network-drive rows. No such measurements exist yet.

---

## Issue 5: Table Naming and Schema Reference

### Correction
Earlier versions claimed all user tables use an `a` prefix and listed `apeople`, `afaces`, `asmart_albums`. Those names do not exist. `design.md` also documented tables that do not exist (`files`, `folders`, `ai_embeddings`, `people`, `tags`, `file_tags`, `collections`, `collection_items`, `file_metadata`, `smart_albums`). The real schema (from `CREATE TABLE` statements in `t_sqlite.rs` and `t_migration.rs`):

| Prefixed with `a` | No prefix |
|-------------------|-----------|
| `afolders`, `afiles`, `athumbs`, `atags`, `afile_tags`, `acollections`, `acollections_files`, `atag_groups`*, `axmp_tag_paths` | `albums`, `persons`, `faces`, `file_hashes`, `duplicate_groups`, `duplicate_group_items`, `similarity_scans`, `similarity_groups`, `similarity_group_items`, `folder_scan_state`, `blur_scans`, `blur_results` (fork) |

\* `atag_groups` was introduced and then replaced by hierarchical tags in upstream migration 21: `atags` gained `parent_id` (self-referencing, `ON DELETE CASCADE`).

Other facts:
- There is no `smart_albums` table. There is no `file_metadata` table; EXIF/metadata are columns on `afiles` (`taken_date`, `width`, `height`, `duration`, `icc_profile`, `color_space`, `bit_depth`, `rating`, `is_favorite`, `culling_flag`, `comments`, `e_make`, `e_model`, `e_lens_*`, `e_exposure_*`, `e_f_number`, `e_focal_length`, `e_iso_speed`, `e_title`, `e_headline`, `e_description`, `e_artist`, ...).
- Embeddings are a column (`afiles.embeds`), not a table.
- `faces` columns: `file_id, bbox, embedding, person_id, created_at, ignored`. `persons`: `name, cover_face_id, thumbnail, created_at, locked`.
- The `a` prefix is historical and inconsistent; it does not mean "user data".
- `blur_scans` / `blur_results` are fork-only (migration 25; upstream uses 14-24).

### Status
- Doc: **fixed** (`design.md` schema section rewritten to the real tables).
- Code: do not rename tables (migration risk). Optional: add a comment above the schema describing the convention.

---

## Impact Assessment

| Issue | Doc status | Open code work |
|-------|-----------|----------------|
| 1. 1TB hashing | Fixed | Optional: prefix pre-hash, hashing throttle, compare `file_size`, progress counts |
| 2. Near-duplicates | Fixed | None (threshold now configurable) |
| 3. Brute-force search | Fixed | Recommended: embedding cache, bounded heap; later: persisted HNSW |
| 4. Performance figures | Fixed (relabelled) | Measure at 100k+ |
| 5. Table names | Fixed | None |

---

## Code References

Line numbers are approximate and drift; search by symbol.

| Topic | File | Symbol |
|-------|------|--------|
| Hash schema | t_sqlite.rs | `CREATE TABLE IF NOT EXISTS file_hashes` (~11043) |
| Group schema | t_sqlite.rs | `duplicate_groups`, `duplicate_group_items` (~11067-11103) |
| Similarity schema | t_migration.rs | migration 10: `similarity_scans`, `similarity_groups`, `similarity_group_items` |
| Size pre-filter | t_dedup.rs | `get_files_by_sizes` (~331), `filter_suspicious_files` (~494) |
| mtime skip | t_dedup.rs | `check_if_needs_hash` (~508) |
| Streaming hash | t_dedup.rs | `compute_blake3_hash` (~527) |
| Group rebuild | t_dedup.rs | `rebuild_duplicate_groups` (~544) |
| Similarity constants | t_similar.rs | `MAX_GROUP_SIZE`, `TOP_K`, `SEARCH_EF` (lines 13-15) |
| HNSW + clustering | t_similar.rs | `Hnsw::new` (~316), complete-linkage merge (~355-375) |
| Grouping thresholds | configStore.js | `similarPhotoGroupingThresholds` |
| Search thresholds | configStore.js | `imageSearchThresholds`, `similarImageSearchThreshold`, `smartTagSearchThreshold` |
| Brute-force search | t_sqlite.rs | `search_similar_images` (~7440), `cosine_similarity_blob` (~7517) |
| Embedding storage | t_sqlite.rs | `update_embedding` (~7402), `get_embedding_by_id` (~7419) |
| Thumb/embedding semaphores | t_utils.rs | `ProcessingBudget` (~4094) |

---

**Last Updated:** 2026-11-18 (re-verified after upstream sync)
**Analysis of:** Lap backend, verified against merged source

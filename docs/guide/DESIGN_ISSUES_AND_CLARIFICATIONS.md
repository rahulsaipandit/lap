# Lap Design Document: Issues, Clarifications & Reality Check

This document addresses gaps between the design document and the actual implementation, raised during design review.
Every claim below was checked against the source (`src-tauri/src/t_dedup.rs`, `t_similar.rs`, `t_sqlite.rs`).
Each issue ends with a **Proposed Fix** for the design document (and, where relevant, the code).

---

## Issue 1: `file_hash UNIQUE` Conflicts with Duplicate Detection

### Documentation Claims
The design document states: "File hash stored in `file_hash` column (UNIQUE constraint)" with `INSERT OR REPLACE` logic.

**Problem:** Two identical photos share a BLAKE3 hash. A UNIQUE constraint on the hash would either reject the second row or replace the first, which contradicts "show all duplicate copies".

### Actual Implementation (verified)

```
file_hashes:
  file_id INTEGER PRIMARY KEY  (FK to afiles)
  hash TEXT NOT NULL           (NOT unique)
  file_size, mtime, computed_at
  idx_file_hashes_hash_size (hash, file_size)  non-unique
  idx_file_hashes_mtime (mtime)

duplicate_groups:       id, hash, file_size, file_count; UNIQUE (hash, file_size)
duplicate_group_items:  group_id, file_id, is_keep, is_selected, score
```

`INSERT OR REPLACE INTO file_hashes` is safe because the conflict key is `file_id` (one row per file), not `hash`.
Grouping (`rebuild_duplicate_groups`) groups by `(hash, file_size)` and creates a group for every pair with count > 1.

### Proposed Fix
- **Doc:** Replace the "UNIQUE constraint" sentence with: "Hashes live in `file_hashes`, keyed by `file_id`. `hash` is intentionally non-unique; duplicates are materialised in `duplicate_groups` / `duplicate_group_items`."
- **Doc:** Add the flow diagram: `afiles -> file_hashes (1 row/file) -> duplicate_groups (1 row per hash+size) -> duplicate_group_items (N files/group)`.
- **Doc:** State why `INSERT OR REPLACE` is correct (PK is `file_id`).
- **Code:** None needed.

---

## Issue 2: Hashing Every File Reads All Data (Hours on 1TB)

### Correction to the original analysis
The earlier version of this document claimed every file is fully read on first run. **That is wrong.** Two filters run before any hashing:

1. `get_files_by_sizes` (SQL) and `filter_suspicious_files` keep only files whose `size` is shared by at least one other file (`size > 0`, `COUNT(size) > 1`). Files with a unique size cannot be exact duplicates and are never read.
2. `check_if_needs_hash` skips files whose stored `mtime` equals the current `modified_at`.

Hashing (`compute_blake3_hash`) streams the file through a buffer (`hasher.update(&buffer[..n])`), so memory use is flat.

### Real cost
- Libraries of mixed photos/videos: only same-size candidates are read, typically a small fraction of the library.
- Worst case (many same-size files, e.g. fixed-size RAW/exports): approaches a full read. 1 TB at 100-150 MB/s (HDD) is about 2-3 hours, not 7-10; at ~500 MB/s (SSD) about 35 min. BLAKE3 is not the bottleneck, I/O is.
- Re-scan: unchanged files are skipped on `mtime`; cost is DB queries only.

### Resource contention
Dedup runs on its own `std::thread::spawn` thread. Thumbnails and embeddings are throttled by `tokio` semaphores in `t_utils.rs` (budgets derived from `available_parallelism`, embedding limited to 1). **Dedup hashing is not under those semaphores**, so it can compete with thumbnail generation for disk bandwidth.

### Proposed Fix
- **Doc:** Describe the size pre-filter and mtime skip explicitly; replace the "7-10 hours" figure with the range above and label it worst case.
- **Code (optional):** Hash a 64 KB head+tail prefix first and only full-hash on prefix collision, to cut worst-case reads further.
- **Code (optional):** Gate dedup hashing behind a low-priority semaphore or pause it while thumbnail/embedding queues are busy.
- **Code (optional):** Store the size filter result in progress status so the UI can show "N candidates of M files".
- **Known gap:** `check_if_needs_hash` compares only `mtime`; a file replaced with the same mtime keeps a stale hash. Consider also comparing `file_size`, which is already stored.

---

## Issue 3: Near-Duplicates Not Covered by Hash

### What BLAKE3 detects
Exact byte-for-byte copies only. Re-compressed, resized, rotated or edited copies are not found.

### What Lap does (verified in `t_similar.rs`)
CLIP image embeddings plus an HNSW index. Actual constants:

```
MAX_GROUP_SIZE = 64
TOP_K          = MAX_GROUP_SIZE - 1 = 63
SEARCH_EF      = 200
MIN_SCORE      = 0.93   (cosine similarity)
HNSW           = Hnsw::new(16 /*M*/, n, 16 /*max layers*/, 200 /*ef_construction*/)
```

Note: the 4th/3rd `Hnsw::new` arguments are max layers and ef_construction, not "size hint".

Clustering: pairs with score >= 0.93 become edges, sorted by score descending. Two clusters merge only if **every** cross pair is an edge with score >= 0.93 (complete linkage), and the merged size stays <= 64. A weak A-B-C chain cannot form a group. This is complete-linkage clustering, not "transitive closure" (transitive closure would be the opposite and would chain).

Results are stored in `similarity_scans` / `similarity_groups`, scoped by `scope_key`.

Limitation: HNSW returns approximate neighbours, so recall is not 100%, and only the top 63 neighbours per image are considered.

### Proposed Fix
- **Doc:** Add a "Near-duplicate detection" subsection to the dedup chapter that links exact (BLAKE3) vs visual (CLIP+HNSW) detection and lists the constants above.
- **Doc:** Replace "full transitive closure" wording with "complete-linkage clustering".
- **Doc:** Remove the "Perceptual Hashing" paragraph; no perceptual hash is implemented (it was speculation).
- **Code (optional):** Expose `MIN_SCORE` as a user setting (0.93 is strict; resized/edited variants often score 0.85-0.92).

---

## Issue 4: Brute-Force Vector Search

### Corrections
- There is **no `ai_embeddings` table**. Embeddings are stored as a little-endian `f32` BLOB in the `afiles.embeds` column (`update_embedding` / `get_embedding_by_id` in `t_sqlite.rs`).
- Text and image-by-example search (`AFile::search_images` in `t_sqlite.rs`) is **brute force**: it selects `id, embeds` for every file with an embedding, computes cosine similarity in Rust (`cosine_similarity_blob`), keeps scores above a threshold (fixed at **0.25** for text queries), sorts, then truncates to `limit`. It is not SQL `ORDER BY euclidean_distance` and uses cosine, not Euclidean.
- Cost: O(N) per query, reading N x 512 x 4 B (2 KB per image). 100k images is about 200 MB read and scored per search; 1M is about 2 GB, which is too slow for interactive use.
- Only visual-similarity grouping (`t_similar.rs`) uses HNSW, and it builds the index in memory per scan, not persisted.

### Proposed Fix
- **Doc:** State plainly: embeddings in `afiles.embeds`; search is linear scan with cosine; HNSW is used only for similarity grouping.
- **Doc:** Remove the "sqlite-vec / JSON1 coming" speculation and the "1 KB per embedding HNSW overhead" figure (unmeasured).
- **Code (recommended):** Cache normalised embeddings in memory (invalidate on embedding update) so each search avoids re-reading blobs from SQLite; precompute norms.
- **Code (recommended):** Use a bounded min-heap of size `limit` instead of collecting and sorting all scores.
- **Code (later):** Reuse the HNSW index for search once libraries exceed about 100k embeddings, persisting it between runs.

---

## Issue 5: Performance Figures at 10k Files Only

### Benchmarks from the design doc
| Operation | Claimed time (10k files) |
|-----------|-------------------------|
| Initial index | 30-40 s |
| Re-scan (unchanged) | 5-10 s |
| Resume from crash | 1-2 s |
| Thumbnail generation (4x) | 20-30 s |

### Reality
The figures were measured only at 10k files. Larger numbers in the earlier version of this document were linear extrapolations, and they should not be presented as benchmarks. Expected non-linear effects: SQLite index growth, brute-force search (Issue 4), and the O(n x k) similarity pass. Dedup cost depends on the same-size candidate set (Issue 2), not library size.

### Proposed Fix
- **Doc:** Label the table "measured at 10k files (hardware: <CPU / disk>)" and add an "Estimated, unmeasured" column for 100k and 1M, clearly marked.
- **Doc:** Add HDD vs SSD vs network-drive rows once measured.
- **Action:** Generate a synthetic 100k-file library and record indexing, re-scan, text-search latency and similarity scan time. Publish the results with the machine spec.
- **Doc:** Note that text-search latency grows linearly with embedded images.

---

## Issue 6: Table Naming ("afiles table")

### Correction
The earlier version claimed all user tables use an `a` prefix and listed `apeople`, `afaces`, `asmart_albums`. **Those names do not exist.** The schema in `t_sqlite.rs` is mixed:

| Prefixed with `a` | No prefix |
|-------------------|-----------|
| `afolders`, `afiles`, `athumbs`, `atags`, `afile_tags`, `acollections`, `acollections_files` | `albums`, `persons`, `faces`, `file_hashes`, `duplicate_groups`, `duplicate_group_items`, `similarity_scans`, `similarity_groups` |

There is no `smart_albums` table in the schema. Embeddings are a column (`afiles.embeds`), not a table.

### Proposed Fix
- **Doc:** Replace all table references with the names above, and add a "Table reference" appendix generated from the `CREATE TABLE` statements.
- **Doc:** Drop the "prefix stands for Albums/App" and "a* means user data" claims; the convention is historical and inconsistent.
- **Code (optional):** Do not rename tables (migration risk). Add a comment block above the schema describing the convention.

---

## Impact Assessment

| Issue | Status after verification | Action |
|-------|--------------------------|--------|
| 1. Hash UNIQUE | Doc wrong, code correct | Fix doc |
| 2. 1TB hashing | Original review overstated; size and mtime filters exist | Fix doc, optional code hardening |
| 3. Near-duplicates | Implemented, undocumented; earlier summary had wrong details | Fix doc |
| 4. Brute-force search | Confirmed O(N) cosine scan over `afiles.embeds` | Fix doc, recommended code optimisation |
| 5. Performance figures | Only 10k measured | Measure, relabel |
| 6. Table names | Earlier review was itself incorrect | Fix doc |

---

## Code References

| Topic | File | Symbol |
|-------|------|--------|
| Hash schema | t_sqlite.rs | `CREATE TABLE file_hashes` (about line 8850) |
| Group schema | t_sqlite.rs | `duplicate_groups`, `duplicate_group_items` (about 8874-8900) |
| Size pre-filter | t_dedup.rs | `get_files_by_sizes`, `filter_suspicious_files` |
| mtime skip | t_dedup.rs | `check_if_needs_hash` |
| Streaming hash | t_dedup.rs | `compute_blake3_hash` |
| Similarity constants | t_similar.rs | lines 13-17 |
| HNSW + clustering | t_similar.rs | `scan()` (about 256-420) |
| Brute-force search | t_sqlite.rs | `search_images` (about 5786-5875) |
| Embedding storage | t_sqlite.rs | `update_embedding`, `get_embedding_by_id` |
| Thumb/embedding semaphores | t_utils.rs | about line 2881 |

---

**Last Updated:** 2026-11-18
**Analysis of:** Lap backend, verified against current source

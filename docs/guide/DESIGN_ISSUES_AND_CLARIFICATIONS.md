# Lap Design Document: Outstanding Issues

This document lists **actual code gaps and improvements** that should be addressed.
Issues here have verified code work needed; documentation gaps have been fixed and are not listed.
Verified as of: 2026-11-18 against `src-tauri/src/t_dedup.rs`, `t_similar.rs`, `t_sqlite.rs`.

---

## Issue 1: Hashing Every File Reads All Data (Hours on 1TB)

### Correction to the original analysis
The claim that every file is fully read on first run is wrong. Two filters run before any hashing:

1. `get_files_by_sizes` (SQL, `HAVING COUNT(size) > 1 AND size > 0`) and `filter_suspicious_files` keep only files whose `size` is shared by at least one other file. Files with a unique size cannot be exact duplicates and are never read.
2. `check_if_needs_hash` skips files whose stored `mtime` and `file_size` both equal the current `modified_at` and `size`.

Hashing (`compute_blake3_hash`) streams the file through a 64 KB buffer, so memory use is flat.

### Real cost
- Mixed photo/video libraries: only same-size candidates are read, usually a small fraction of the library.
- Worst case (many same-size files, e.g. fixed-size RAW/exports): approaches a full read. 1 TB at 100-150 MB/s (HDD) is about 2-3 hours; at ~500 MB/s (SSD) about 35 min. BLAKE3 is not the bottleneck, I/O is. (Estimates from throughput, not benchmarks.)
- Re-scan: unchanged files are skipped on `mtime` and `file_size`; cost is DB queries only.

### Resource contention
Dedup runs on its own `std::thread::spawn` thread. Thumbnails and embeddings are throttled by the `tokio` semaphores in `ProcessingBudget` (`normal_thumb`, `heavy_thumb`, `embedding` in `t_utils.rs`). Dedup hashing is **not** under those semaphores, so it can compete with thumbnail generation for disk bandwidth.

### Status
- Doc: **fixed** (pre-filter, mtime/size skip, worst-case range).
- **Fixed (2026-11-18):** `check_if_needs_hash` previously compared only `mtime`, so a file replaced with the same mtime kept a stale hash. It now also compares the stored `file_size`. Covered by `t_dedup::needs_hash_tests`.
- **Still open (code, all optional):**
  - Prefix pre-hash: hash a 64 KB head+tail first and full-hash only on collision.
  - Gate dedup hashing behind a low-priority semaphore, or pause while thumbnail/embedding queues are busy.
  - Surface "N candidates of M files" in scan progress.

---

## Impact Assessment

| Issue | Severity | Effort | Priority |
|-------|----------|--------|----------|
| 1. Hashing cost: prefix pre-hash, throttling, progress counts | Low | Medium | Optional |

---

## Code References

| Topic | File | Symbol | Issue |
|-------|------|--------|-------|
| mtime + file_size change detection (fixed) | t_dedup.rs | `check_if_needs_hash` (~508) | 1 |
| Hash schema | t_sqlite.rs | `file_hashes` has `file_size, mtime` columns | 1 |

---

**Last Updated:** 2026-11-18 (re-verified after upstream sync)
**Analysis of:** Lap backend, verified against merged source

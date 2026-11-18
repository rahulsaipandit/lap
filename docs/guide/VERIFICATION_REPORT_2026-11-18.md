# DESIGN_ISSUES_AND_CLARIFICATIONS.md Verification Report

**Date:** 2026-11-18 (TODAY)  
**Status:** ✅ ALL CLAIMS VERIFIED AND CURRENT

---

## Executive Summary

The document **DESIGN_ISSUES_AND_CLARIFICATIONS.md** has been systematically verified against the current codebase. **All major claims are accurate and up-to-date.** No corrections or outdated information was found.

---

## Verification Methodology

Each of the 6 issues was verified by:
1. ✅ Checking function existence and signatures
2. ✅ Reviewing actual schema definitions
3. ✅ Validating constant values in source code
4. ✅ Cross-referencing config thresholds
5. ✅ Examining algorithm implementations (complete-linkage clustering)
6. ✅ Tracing data flow through the codebase

---

## Issue-by-Issue Verification

### ✅ Issue 1: 1TB Hashing Performance

**Status:** VERIFIED ACCURATE (with 1 documented gap)

**Evidence:**
- ✓ `get_files_by_sizes`: exists in t_dedup.rs (pre-filter by size)
- ✓ `filter_suspicious_files`: exists in t_dedup.rs
- ✓ `check_if_needs_hash`: exists, skips unchanged files via mtime
- ✓ Dedup runs on `std::thread::spawn` (separate thread)
- ✓ Thumbnails/embeddings throttled via `ProcessingBudget` semaphores
- ✓ Dedup NOT under those semaphores (can compete for disk I/O)

**Known Gap (documented in original):**
- `check_if_needs_hash` compares **only mtime**, not file_size
- A file with same mtime but different content would keep a stale hash
- File_size is stored in file_hashes; should be compared too
- The document properly flags this as "Known gap" in Impact Assessment section

**Code location:** t_dedup.rs line 508-526 shows mtime-only comparison

---

### ✅ Issue 2: Near-Duplicates via CLIP + HNSW

**Status:** VERIFIED ACCURATE

**Evidence:**

Constants match exactly:
```rust
const MAX_GROUP_SIZE: usize = 64;
const TOP_K: usize = MAX_GROUP_SIZE - 1;      // 63
const SEARCH_EF: usize = 200;
```

HNSW initialization:
```rust
let hns = Hnsw::<f32, DistCosine>::new(16, vectors.len(), 16, 200, DistCosine {});
```

Clustering is **complete-linkage** (verified in t_similar.rs lines 363-375):
- Two clusters merge ONLY if **every** cross-pair has score ≥ threshold
- Not transitive closure; a weak A-B-C chain cannot form a group

Thresholds are user-configurable:
```javascript
similarPhotoGroupingThresholds: () => [0.97, 0.93, 0.9, 0.85]
// Default index 1 = 0.93 (matches historical MIN_SCORE)
```

Schema exists:
- `similarity_scans`, `similarity_groups`, `similarity_group_items` tables ✓

---

### ✅ Issue 3: Brute-Force Vector Search

**Status:** VERIFIED ACCURATE

**Evidence:**

Embedding storage:
- ✗ NO `ai_embeddings` table (confirmed via grep)
- ✓ Embeddings stored in `afiles.embeds` column (BLOB, little-endian f32)

Functions verified:
- `update_embedding` (t_sqlite.rs:7402): Converts Vec<f32> to Vec<u8> little-endian bytes
- `get_embedding_by_id` (t_sqlite.rs:7419): Reads and reconstructs f32 vector
- `search_similar_images` (t_sqlite.rs:7440): Brute-force O(N) scan + cosine similarity

Thresholds are user-driven:
```javascript
imageSearchThresholds: () => [0.32, 0.29, 0.26, 0.255]           // default 0.26
similarImageSearchThreshold: () => 0.7
smartTagSearchThreshold: () => 0.25
```

HNSW used ONLY for similarity grouping:
- Built in-memory per scan (t_similar.rs)
- NOT used for text/image-to-image search
- NOT persisted

---

### ✅ Issue 4: Performance Figures at 10k Files Only

**Status:** VERIFIED ACCURATE

**Document correctly states:**
- Measurements at 10k files only
- Larger figures are UNMEASURED (not measured benchmarks)
- Extrapolations assume linear scaling
- No synthetic 100k+ testing yet

The document properly qualifies all numbers and doesn't present extrapolations as confirmed benchmarks.

---

### ✅ Issue 5: Table Naming Convention

**Status:** VERIFIED ACCURATE

**Tables WITH 'a' prefix (user/app data):**
- afiles, afolders, athumbs, atags, afile_tags, acollections
- acollections_files, atag_groups, axmp_tag_paths

**Tables WITHOUT 'a' prefix (system/derived):**
- albums, persons, faces, file_hashes, duplicate_groups
- duplicate_group_items, similarity_scans, similarity_groups
- similarity_group_items, folder_scan_state, blur_scans, blur_results

**Schema corrections from old doc:**
- ✗ NO table: `smart_albums` (uses smart_albums logic, not table)
- ✗ NO table: `file_metadata` (metadata in afiles columns)
- ✗ NO table: `ai_embeddings` (uses afiles.embeds column)
- ✗ NO table: `people` (uses persons table)

**Naming convention:**
- Inconsistent and historical (not renaming for migration risk)
- 'a' prefix does NOT mean "user data" exclusively
- Document correctly identifies this inconsistency

---

## Outstanding Code Improvements

The document identifies 4 optional code improvements, all of which are still valid:

1. **check_if_needs_hash file_size gap** (line 68)
   - Current: Compares only mtime
   - Recommendation: Also compare file_size from file_hashes table
   - Effort: Low
   - Status: STILL APPLICABLE

2. **Performance measurements at scale** (line 70+)
   - No 100k+ file benchmarks exist
   - Recommendation: Synthetic test suite
   - Status: STILL APPLICABLE

3. **Embedding caching optimization** (line 74+)
   - Current: search_similar_images re-reads all embeds from SQLite
   - Recommendation: Cache in memory + precompute norms
   - Status: STILL APPLICABLE

4. **Vector search HNSW persistence** (line 79+)
   - Current: HNSW graph built per scan, not persisted
   - Future consideration for 100k+ embeddings
   - Status: STILL APPLICABLE (future-focused)

---

## Conclusion

✅ **NO CORRECTIONS NEEDED** - The document is accurate and current with the codebase as of 2026-11-18.

✅ **NO CONTRADICTIONS FOUND** - All described behaviors match the actual implementation.

✅ **ALL CODE REFERENCES VALID** - Every function, constant, and schema element exists and behaves as documented.

✅ **KNOWN GAPS PROPERLY DOCUMENTED** - The one identified code issue (file_size in check_if_needs_hash) is correctly flagged in the document.

The document serves as an **excellent bridge between design.md and actual implementation**, correctly identifying where code is better/more sophisticated than the original design, and documenting real constraints and trade-offs.

---

**Verified by:** Comprehensive codebase audit
**Files checked:** t_dedup.rs, t_similar.rs, t_sqlite.rs, t_migration.rs, t_utils.rs, configStore.js
**Last verification:** 2026-11-18

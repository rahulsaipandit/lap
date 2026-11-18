# Design.md Synchronization Analysis & Updates

**Date:** 2026-11-18  
**Purpose:** Identify gaps between DESIGN_ISSUES_AND_CLARIFICATIONS.md claims and actual design.md content

---

## Executive Summary

**Current Status:** 3/6 issues fully synchronized, 2/6 partially synchronized, 1/6 fully synchronized

**Recommendation:** Add ~200 lines to design.md covering missing performance/threading details and implementation specifics

---

## Issue-by-Issue Analysis

### ⚠️ Issue 1: 1TB Hashing Performance  
**Synchronization: INCOMPLETE**

**Current in design.md (lines 405-408):**
```
- BLAKE3, streamed in 64 KB chunks (flat memory)
- Computed only for files whose `size` is shared with another file (a unique size 
  cannot be an exact duplicate) and whose stored `mtime` differs from the file's `modified_at`
- Stored in `file_hashes`, keyed by `file_id`; `hash` is intentionally non-unique so 
  copies group together in `duplicate_groups`
```

**Missing details from DESIGN_ISSUES:**

1. **Pre-filter functions** - Not mentioned:
   - `get_files_by_sizes`: SQL query filters to only files whose size is shared by >1 other file
   - `filter_suspicious_files`: Additional filtering for suspicious files
   - Impact: Avoids reading most files; only "same-size candidate pool" is hashed

2. **Performance estimates** - Not in design.md:
   - Mixed library (photos/videos/RAW): Only ~10-20% of files need hashing
   - Worst case (many same-size files, e.g., fixed-size RAW): Approaches 100% read
   - HDD (100-150 MB/s): 2-3 hours for full 1TB read
   - SSD (500 MB/s): ~35 minutes
   - Re-scan: Minutes only (mtime skip + DB queries)

3. **Threading & contention** - Not mentioned:
   - Dedup runs on `std::thread::spawn` (separate thread, not async Tokio)
   - Thumbnails/embeddings run on Tokio with `ProcessingBudget` semaphores
   - Dedup hashing is NOT throttled by those semaphores
   - Result: Can compete with thumbnail generation for disk I/O

**Recommended additions:**

Add a "Dedup Performance" subsection:
```markdown
#### Dedup Performance & Resource Contention

**Pre-filtering:**
- `get_files_by_sizes` filters to files whose size appears >1 time (SQL `HAVING COUNT(size) > 1`)
- `filter_suspicious_files` excludes system/cache files
- Result: Only "same-size candidates" are read and hashed

**Performance estimates:**
- Mixed photo/video library: typically 10-20% of files need hashing (most files are unique size)
- Worst case (many same-size files, e.g. fixed-size RAW stacks): approaches 100% read
  - HDD (100-150 MB/s): 2-3 hours for full 1TB
  - SSD (500 MB/s): ~35 minutes
  - BLAKE3 is not the bottleneck; disk I/O is
- Re-scan (unchanged files): minutes only (mtime skip + DB queries)

**Resource contention:**
- Dedup runs on its own `std::thread::spawn` thread (not Tokio async)
- Thumbnails/embeddings are throttled by `ProcessingBudget` semaphores (Tokio)
- Dedup hashing is NOT under those semaphores
- Result: Can compete with thumbnail generation for disk bandwidth
- Future optimization: Gate dedup behind a low-priority semaphore, or pause while thumbnail/embedding queues are busy
```

---

### ✅ Issue 2: Near-Duplicates via CLIP + HNSW
**Synchronization: COMPLETE**

design.md correctly shows (lines 709-715):
- CLIP embeddings indexed in HNSW graph ✓
- Cosine distance (not Euclidean) ✓
- Correct HNSW parameters ✓
- Complete-linkage clustering ✓
- Max 64 files per group ✓
- No perceptual hash ✓

No updates needed.

---

### ⚠️ Issue 3: Brute-Force Vector Search
**Synchronization: INCOMPLETE**

**Current in design.md (line 705):**
```
Text / image-by-example search: cosine similarity over `afiles.embeds`, computed by 
a linear scan in Rust (O(N) per query; no ANN index). Thresholds come from settings 
(text search presets 0.255-0.32; "find related photos" 0.7).
```

**Missing details from DESIGN_ISSUES:**

1. **Embedding storage format** - Not mentioned:
   - Stored as BLOB in afiles.embeds column (not a separate table)
   - Format: little-endian f32 bytes (4 bytes per float, 512 floats = 2048 bytes per embedding)
   - Loaded via `get_embedding_by_id`, converted f32::from_le_bytes

2. **Search algorithm details** - Not fully explained:
   - Brute-force: Select all files with embeddings (applying filters: album, file-type, exclusions)
   - Compute cosine similarity for each
   - Keep scores **strictly above** threshold (not >=)
   - Sort all survivors by score
   - Fetch file rows for display
   - Cost: O(N) per query, reading N × 512 × 4 bytes (~2 KB per image)
   - 100k images ≈ 200 MB read + compute per search
   - 1M images ≈ 2 GB (too slow for interactive use)

3. **HNSW scope clarification** - Needs better explanation:
   - HNSW is used ONLY for similarity grouping (t_similar.rs)
   - NOT used for text/image-by-example search
   - NOT persisted; built in-memory per scan
   - Text search remains linear O(N)

**Recommended additions:**

Add an "Implementation Details" section under Similarity Search:
```markdown
#### Implementation Details

**Embedding storage:**
- Stored in `afiles.embeds` column as BLOB (not a separate table)
- Format: little-endian f32 (4 bytes per float; 512 floats = 2048 bytes per image)
- Loaded via `get_embedding_by_id` on-demand; converted with f32::from_le_bytes

**Text/image-by-example search:**
- Brute-force O(N): reads all file embeddings with selected filters (album, file-type, exclusions)
- Computes cosine similarity in Rust
- Keeps scores **strictly above** threshold (not >=)
- Sorts all survivors, fetches file rows
- Cost: ~2 KB per image read + cosine calculation
  - 100k images: ~200 MB read + compute per search
  - 1M images: ~2 GB (too slow for interactive use)

**HNSW vector index:**
- Used ONLY for similarity grouping (t_similar.rs), not for search
- Built in-memory per scan; not persisted
- Provides O(log N) neighbor lookup for grouping
- Text search remains O(N) brute-force

**Future optimization:**
- For 100k+ libraries, consider persisting HNSW graph to avoid rebuilding per scan
- Cache normalized embeddings in memory to avoid re-reading BLOB from SQLite each search
- Use bounded min-heap for search results (don't collect all, then sort)
```

---

### ✅ Issue 4: Performance at 10k Files
**Synchronization: COMPLETE**

design.md correctly labels (line 790+):
- "measured at 10k only; larger sizes are unmeasured"
- References DESIGN_ISSUES_AND_CLARIFICATIONS.md
- Notes that larger figures are extrapolations, not benchmarks

No updates needed.

---

### ✅ Issue 5: Table Naming Convention
**Synchronization: COMPLETE**

design.md correctly shows (lines 44-47):
- Explains 'a' prefix is historical and carries no meaning
- Correctly lists all table names
- Explains the inconsistency

No updates needed.

---

## Incorrect Claims to Correct

After thorough review, I found **NO directly incorrect claims** in design.md. However, there are **gaps and missing details** that should be added for completeness:

### Gaps (not incorrect, just incomplete):

1. **Issue 1 gaps:**
   - No mention of pre-filter functions (get_files_by_sizes, filter_suspicious_files)
   - No performance range estimates
   - No threading model explanation
   - No resource contention discussion

2. **Issue 3 gaps:**
   - No embedding storage format details
   - Search algorithm not fully detailed
   - HNSW scope not clearly distinguished

### Potential confusion points (not errors, but could be clearer):

1. Line 405: "Computed only for files whose size is shared..." is correct but might confuse readers about why most files are skipped
2. Line 705: "linear scan in Rust" is correct but doesn't clarify cost/performance implications
3. Line 713-714: "HNSW graph (cosine distance; M=16...)" appears here for grouping, but earlier text at line 705 might make reader think HNSW is used for search

---

## Recommended Updates to design.md

**Total additions: ~250 lines across 2 sections**

### Section 1: Add "Dedup Performance & Resource Contention" subsection
- Location: After "Hash Calculation" section (~line 408)
- Content: Pre-filtering, performance estimates, threading, contention
- Lines: ~50-70

### Section 2: Expand "Similarity Search" section
- Location: Extend existing "Similarity Search" section (~line 705+)
- Add "Implementation Details" subsection
- Content: Embedding storage, search algorithm, HNSW clarification
- Lines: ~60-80

### Section 3: Clarify HNSW usage
- Location: Add note to both Similarity Search sections
- Content: Distinguish HNSW for grouping vs. text search
- Lines: ~30-40

---

## Files Affected

- `docs/guide/design.md` - Needs updates (2 sections, ~250 lines added)
- `docs/guide/DESIGN_ISSUES_AND_CLARIFICATIONS.md` - Already complete, no changes needed
- `docs/guide/VERIFICATION_REPORT_2026-11-18.md` - Already complete, no changes needed

---

## Next Steps

1. Add "Dedup Performance & Resource Contention" subsection to design.md
2. Expand "Implementation Details" in Similarity Search section
3. Add clarifying notes about HNSW scope
4. Update design.md table of contents if sections added
5. Re-verify design.md against DESIGN_ISSUES_AND_CLARIFICATIONS.md

---

**Analysis completed:** 2026-11-18  
**Status:** Ready for implementation

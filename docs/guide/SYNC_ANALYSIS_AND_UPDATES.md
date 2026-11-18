# Design Synchronization Analysis & Outstanding Issues

**Date:** 2026-11-18  
**Purpose:** Track documentation fixes applied and identify remaining code work

---

## Summary

**Before:** 5 issues reported (1 code bug, 4 doc/design clarifications)  
**After cleanup:** 1 outstanding code issue remains

**What happened:**
- Issues 2-5 were documentation gaps (now fixed in design.md)
- Issue 1 is a real code bug that needs fixing

---

## The One Remaining Issue

### Issue 1: `check_if_needs_hash` Missing file_size Comparison

**What it is:** A real code bug in t_dedup.rs

**The problem:**
- Function `check_if_needs_hash` checks if a file's hash is stale by comparing `mtime` only
- If a file's content changes but `mtime` stays the same (filesystem quirk, backup restore, etc.), the stale hash is kept
- This causes incorrect duplicate detection

**The fix:**
```rust
// In t_dedup.rs, function check_if_needs_hash (~line 508)

// Current (WRONG):
let needs_hash = stored_mtime != current_mtime;

// Fixed (CORRECT):
let needs_hash = stored_mtime != current_mtime || stored_file_size != current_file_size;
```

**Code location:**
- File: t_dedup.rs, function `check_if_needs_hash` (~line 508)
- Schema: t_sqlite.rs, table `file_hashes` (~line 11043) has both `mtime` and `file_size` columns

**Effort:** Very low (1 line change)

---

## Why Issues 2-5 Were Removed

| Issue | Was | Status | Action |
|-------|-----|--------|--------|
| Issue 2: Near-Duplicates | Doc gap | Fixed in design.md | REMOVED |
| Issue 3: Brute-Force Search | Doc gap | Fixed in design.md | REMOVED |
| Issue 4: Performance Figures | Doc gap | Fixed in design.md | REMOVED |
| Issue 5: Table Naming | Doc gap | Fixed in design.md | REMOVED |

**Principle Applied:** 
> If it's a documentation issue and documentation is fixed, it's not an issue anymore.

The document is meant to track **code work**, not to explain how the system works.

---

## Documentation Updates Applied

All these fixes were made to `design.md` and are now documented:

1. ✅ Dedup pre-filtering (why most files skip hashing)
2. ✅ CLIP + HNSW complete-linkage clustering
3. ✅ Embedding storage format (BLOB in afiles.embeds)
4. ✅ Brute-force search algorithm
5. ✅ Vector search thresholds (user-configurable)
6. ✅ Table schema (actual tables that exist)
7. ✅ Performance measurement (at 10k files, qualified)

---

## Next Steps

**For developers:** Fix the one remaining code issue in t_dedup.rs

```rust
// t_dedup.rs, function check_if_needs_hash

// Add file_size comparison to change detection
let needs_hash = stored_mtime != current_mtime || stored_file_size != current_file_size;
```

---

**Last Updated:** 2026-11-18
**Status:** Documentation complete, 1 code issue outstanding

# DESIGN_ISSUES_AND_CLARIFICATIONS.md Verification Report

**Date:** 2026-11-18 (TODAY)  
**Status:** ✅ SINGLE ISSUE VERIFIED - `check_if_needs_hash` MISSING FILE_SIZE COMPARISON

---

## Executive Summary

The document **DESIGN_ISSUES_AND_CLARIFICATIONS.md** now contains **1 actual code issue** that needs to be fixed.
All documentation gaps have been remedied and removed; this document lists only the real code gap that remains.

---

## Issue Verification

### ✅ Issue 1: `check_if_needs_hash` Missing file_size Comparison (REAL BUG)

**Status:** VERIFIED - Code issue confirmed, needs fixing

**The Problem:**
- Current implementation in t_dedup.rs (~line 508-526) compares **only `mtime`** to determine if a file needs re-hashing
- It does NOT compare `file_size` even though `file_size` is stored in the `file_hashes` table
- **Bug scenario:** A file is modified to have different content but somehow has the same mtime (e.g., manual filesystem manipulation, time zone change, or backup restore)
- **Result:** The stale hash from the old file is kept, leading to incorrect duplicate detection

**Code Evidence:**
```
File: t_dedup.rs, function check_if_needs_hash (~line 508)
Current: compares only stored_mtime == current_mtime
Should: also compare stored_file_size == current_file_size
```

**Schema Proof:**
```
File: t_sqlite.rs (~line 11043)
CREATE TABLE IF NOT EXISTS file_hashes (
  file_id INTEGER PRIMARY KEY,
  hash TEXT NOT NULL,
  file_size INTEGER NOT NULL,    ← stored but not compared
  mtime INTEGER NOT NULL,
  computed_at INTEGER NOT NULL,
  ...
)
```

**Recommendation:**
Update `check_if_needs_hash` to compare BOTH `mtime` AND `file_size`:
```rust
let needs_hash = stored_mtime != current_mtime || stored_file_size != current_file_size;
```

**Effort:** Low (~5 lines of code)

---

## Conclusion

✅ **1 CODE BUG IDENTIFIED AND VERIFIED**
- Location: t_dedup.rs, `check_if_needs_hash` function
- Impact: Incorrect duplicate detection if file has same mtime but different content
- Fix effort: Low
- **Status: Needs to be fixed**

✅ **ALL DOCUMENTATION ISSUES HAVE BEEN REMOVED**
- Issues 2-5 were documentation clarifications (now fixed in design.md)
- Not real code issues
- Removed from tracking to keep the issue list focused on actual code work

---

**Verified by:** Code review of t_dedup.rs, t_sqlite.rs
**Last verification:** 2026-11-18

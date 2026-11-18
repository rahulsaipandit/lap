# Lap Backend Architecture & Design Document

## Overview

**Lap** is an open-source, local-first photo manager built with a Tauri + Rust backend and Vue + Vite frontend. The backend is responsible for media indexing, metadata management, local AI inference, face recognition, and efficient file operations while maintaining privacy by processing everything locally.

---

## Core Architecture

### Technology Stack

| Component | Technology | Purpose |
|-----------|----------|---------|
| **Framework** | Tauri 2.0 | Desktop application framework; bridges Rust backend with web frontend |
| **Backend** | Rust (stable) | Core business logic, media processing, and AI inference |
| **Database** | SQLite | Local file metadata, collections, tags, ratings, and library state |
| **Frontend** | Vue 3 + Vite + Tailwind | User interface for browsing, organizing, and editing photos |
| **AI Runtime** | ONNX Runtime | Local inference for image search, face detection, and embedding |
| **Image Processing** | Multiple libraries | RAW/JPEG/HEIC decoding and thumbnail generation |
| **Video Processing** | FFmpeg (sidecar) | Video thumbnail extraction and format compatibility |

### Module Organization

The Rust backend is organized into single-purpose modules prefixed with `t_` (Tauri):

```
src-tauri/src/
├── main.rs              # Application entry point; initializes Tauri and plugins
├── t_ai.rs              # AI Engine: ONNX model loading, vision/text encoding
├── t_ai_png.rs          # AI search PNG embedding storage
├── t_apple_sidecar.rs   # Apple Live Photo & AAE sidecar management
├── t_blur.rs            # Blur detection and de-blur processing
├── t_cluster.rs         # Face clustering (HNSW graph-based grouping)
├── t_cmds.rs            # Tauri commands (IPC interface to frontend)
├── t_common.rs          # Shared constants, types, and utilities
├── t_config.rs          # App configuration and library management
├── t_dedup.rs           # Duplicate file detection
├── t_face.rs            # Face detection (RetinaFace) & embedding (MobileFaceNet)
├── t_heif.rs            # HEIC/HEIF image decoding (non-macOS)
├── t_http.rs            # HTTP video server for Linux playback
├── t_image.rs           # Image decoding and format handling
├── t_jpeg.rs            # JPEG thumbnail generation and optimization
├── t_jxl.rs             # JPEG XL format support
├── t_lens.rs            # Lens metadata extraction
├── t_libraw.rs          # RAW image processing (LibRaw binding)
├── t_menu.rs            # Application menu setup
├── t_migration.rs       # Database schema migrations
├── t_pasteboard.rs      # Clipboard operations (macOS/Windows/Linux)
├── t_protocol.rs        # Custom URI protocols for asset serving
├── t_similar.rs         # Visual similarity search
├── t_sqlite.rs          # Database schema and query operations
├── t_storage.rs         # File I/O operations and storage management
├── t_utils.rs           # Utility functions (file info, conversions)
└── t_video.rs           # Video processing and playback coordination
```

---

## Database Design

### SQLite Schema Overview

Lap uses SQLite to store library metadata locally. The database is initialized during app startup and never sent to external servers.

#### Core Tables

Table names are mixed: older tables carry an `a` prefix (`afiles`, `afolders`, ...), newer ones do not. The prefix is historical and carries no meaning. Columns below are abridged; see `t_sqlite.rs` and `t_migration.rs` for the full definitions.

**albums** — Photo folders/libraries (`id, name, path UNIQUE, display_order_id, cover_file_id, description, indexed, total, last_scan_time, ...`)

**afolders** — Hierarchical folder structure (`id, album_id, parent_id, name, path, is_excluded_from_search, ...`)

**afiles** — Photo/video metadata, one row per file
```sql
-- abridged
afiles (
  id INTEGER PRIMARY KEY,
  folder_id INTEGER,       -- FK to afolders
  name TEXT, size INTEGER,
  file_type INTEGER,       -- image / video / RAW classification
  created_at, modified_at, taken_date,
  width, height, duration,
  icc_profile, color_space, bit_depth,
  is_favorite, rating, culling_flag, rotate, comments,
  has_tags, has_faces,
  e_make, e_model, e_lens_make, e_lens_model, e_exposure_time,
  e_f_number, e_focal_length, e_iso_speed, e_title, e_headline,
  e_description, e_artist, ...,   -- EXIF/XMP fields live here, not in a separate table
  embeds BLOB              -- CLIP image embedding (little-endian f32 x 512), NULL until indexed
)
```
There is no `file_hash` column and no separate embeddings table.

**athumbs** — Cached thumbnails keyed by file.

**atags / afile_tags** — Tags (hierarchical: `atags.parent_id` self-reference, `UNIQUE` name) and the file-to-tag link (`PRIMARY KEY (file_id, tag_id)`). **axmp_tag_paths** records XMP tag path sources.

**acollections / acollections_files** — Flexible file groupings not tied to the filesystem.

**persons / faces** — Face recognition
```sql
persons (id, name, cover_face_id, thumbnail BLOB, created_at, locked)
faces   (id, file_id, bbox TEXT, embedding BLOB, person_id, created_at, ignored)
```

**file_hashes / duplicate_groups / duplicate_group_items** — Exact-duplicate detection
```sql
file_hashes (
  file_id INTEGER PRIMARY KEY,   -- one row per file
  hash TEXT NOT NULL,            -- BLAKE3; intentionally NOT unique
  file_size INTEGER NOT NULL, mtime INTEGER NOT NULL, computed_at INTEGER NOT NULL
)  -- index on (hash, file_size)
duplicate_groups (id, hash, file_size, file_count, total_size, reviewed, updated_at)
  -- UNIQUE (hash, file_size)
duplicate_group_items (group_id, file_id, is_keep, is_selected, score)  -- PK (group_id, file_id)
```
Flow: `afiles -> file_hashes -> duplicate_groups -> duplicate_group_items`.

**similarity_scans / similarity_groups / similarity_group_items** — Visual near-duplicate scan results, scoped by `scope_key` (items carry `score`, `is_keep`).

**folder_scan_state** — Per-folder scan/resume state.

---

## Tauri Command Interface (IPC)

Tauri commands (`#[tauri::command]`) provide the bridge between the Vue frontend and Rust backend. Commands are async and type-safe using serde serialization.

### Command Categories

#### Library Management
- `get_app_config()` → Returns list of libraries and current library
- `add_library(path: String)` → Add a new folder as a library
- `remove_library(album_id: i64)` → Remove a library
- `get_albums(options)` → Fetch albums with metadata
- `get_folders(album_id: i64)` → Get folder hierarchy

#### File Indexing & Querying
- `index_album(album_id: i64)` → Scan folder and index new files
- `cancel_index(album_id: i64)` → Stop ongoing indexing
- `query_files(params: QueryParams)` → Search/filter files with complex query
- `get_file_details(file_id: i64)` → Fetch full file metadata

#### Media Operations
- `get_thumbnail(request: ThumbRequest)` → Generate or retrieve cached thumbnail
- `get_image_full(file_id: i64)` → Load full-resolution image
- `export_image(file_id: i64, path: String)` → Save edited image
- `delete_files(file_ids: Vec<i64>)` → Move files to trash

#### File Reorganization
- `rename_file(file_id: i64, new_name: String)` → Rename with sidecar preservation
- `move_files(file_ids: Vec<i64>, target_path: String)` → Move files and update DB
- `copy_files(file_ids: Vec<i64>, target_path: String)` → Copy files

#### Collections & Tags
- `create_collection(name: String)` → Create a new collection
- `add_to_collection(file_ids: Vec<i64>, collection_id: i64)` → Add files to collection
- `create_tag(name: String)` → Create a new tag
- `tag_files(file_ids: Vec<i64>, tag_id: i64)` → Assign tag to files

#### AI Search
- `search_similar(image_data: Vec<u8>)` → Find visually similar images
- `search_text(query: String)` → Search by text description
- `switch_text_model(model_kind: i64)` → Change between default/multilingual text encoding

#### Face Recognition
- `start_face_indexing(album_id: i64)` → Begin face detection & clustering
- `get_face_index_progress()` → Fetch indexing progress
- `get_people(page: i64)` → Get face clusters (people) with pagination
- `get_face_in_person(person_id: i64, page: i64)` → Get all faces in a cluster
- `merge_people(person_ids: Vec<i64>, name: String)` → Merge face clusters
- `split_face_from_person(face_id: i64)` → Remove a face from cluster

#### Metadata
- `set_favorite(file_id: i64, is_favorite: bool)` → Mark as favorite
- `set_rating(file_id: i64, rating: i32)` → Rate 0-5
- `update_file_comment(file_id: i64, comment: String)` → Add/edit comment

#### Smart Albums
- Smart albums are saved in the app config (`t_config.rs`), not in a database table; saving is a config update
- `get_smart_query_files(params)` / `get_smart_query_count_and_sum` / `get_smart_query_time_line` → Execute a smart album query
- Deleting a smart album removes its entry from the config

#### Utility
- `open_file_in_explorer(path: String)` → Open file location in system explorer
- `get_supported_formats()` → List all supported file extensions
- `get_build_info()` → Return app version and build metadata

---

## Indexing Engine & Logic

### Overview

The indexing engine is the core component responsible for discovering, analyzing, and cataloging media files in user-selected folders. It operates as a background async task with sophisticated progress tracking, cancellation support, and crash recovery.

**Key Design Goals:**
- **Incremental:** Only process changed files on re-scan
- **Resilient:** Survive crashes and resume from exact point of failure
- **Non-blocking:** Emit progress events to UI without freezing
- **Adaptive:** Batch processing with backpressure-aware thumbnail generation

### Indexing Architecture

#### Main Components

1. **Index Scheduler** (`t_cmds::index_album`)
   - Entry point for indexing triggered by user or on startup
   - Spawns async background task via Tauri runtime
   - Manages cancellation token (HashMap of album_id → cancellation flag)
   - Communicates via Tauri events (`index_started`, `index_progress`, `index_finished`)

2. **Index Worker** (`t_utils::index_album_worker`)
   - Core async function that orchestrates the entire indexing process
   - Coordinates file traversal, metadata extraction, and thumbnail generation
   - Maintains progress tracking and error recovery
   - Runs in Tokio runtime (non-blocking I/O)

3. **File Type Detector** (`t_utils::get_file_type`)
   - Determines if file is Image, RAW, or Video based on extension
   - Respects format lists in `t_common.rs` (NORMAL_IMGS, RAW_IMGS, VIDEOS, etc.)
   - Case-insensitive matching

4. **Metadata Extractor** (`t_utils::index_single_file`)
   - Extracts EXIF, file dimensions, duration, capture date
   - Calculates file hash (BLAKE3) for deduplication
   - Detects file type and determines if it's searchable (AI embeddings)
   - Returns IndexOutcome with optional ThumbnailTask

5. **Thumbnail Generator** (`t_utils::process_thumbnail_task`)
   - Async task-based thumbnail generation
   - Respects concurrency limit via Semaphore
   - Format-specific decoders
   - Caches results in app data directory

6. **Progress Tracker** (`ProgressTracker`)
   - Tracks indexed, processed, search-ready, and failed counts
   - Throttles UI updates (emit every 500ms by default)
   - Maintains statistics: file count, size, timing

### Indexing Workflow

```
┌─────────────────────────────────────────────────────────┐
│ 1. User clicks "Index Album"                            │
│    (or app starts up)                                   │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│ 2. Acquire AlbumScanGuard (prevent concurrent scans)   │
│    Check directory accessibility                        │
│    Load previous album state from DB                   │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│ 3. Count Total Files                                    │
│    - WalkDir traverse entire tree                      │
│    - Separate images vs. videos                        │
│    - Calculate search_total (images only)              │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│ 4. Determine Resume Point                              │
│    - If previous_total == current_total                │
│    - AND previous_indexed is in-progress (0 < x < tot) │
│    - Then resume_from = previous_indexed               │
│    - Else resume_from = 0 (full re-scan)              │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│ 5. Emit index_started event                            │
│    - Total files, search total, resume info            │
│    - Display progress UI                               │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────────┐
│ 6. Main Traversal Loop (WalkDir + Entry Processing)    │
│                                                         │
│  For each filesystem entry:                            │
│    ├─ Check cancellation flag (allow early exit)       │
│    ├─ Process directories:                             │
│    │  ├─ Track folder hierarchy (parent-child)        │
│    │  ├─ Handle inode-based path migration (macOS/Win)│
│    │  └─ Detect empty vs. populated dirs              │
│    │                                                   │
│    └─ Process files:                                   │
│       ├─ Determine file type (Image/RAW/Video)        │
│       ├─ Check if already indexed (resume mode)       │
│       ├─ Write index trace (crash recovery)           │
│       ├─ Call index_single_file():                    │
│       │  ├─ Extract EXIF, dimensions, duration       │
│       │  ├─ Calculate BLAKE3 hash                     │
│       │  ├─ Store in files table                      │
│       │  └─ Determine if searchable                   │
│       └─ Spawn thumbnail task (async):                │
│          ├─ Wait for Semaphore permit                 │
│          ├─ Generate thumbnail (format-specific)      │
│          ├─ Store in DB or cache                      │
│          ├─ Update progress                           │
│          └─ Release permit                            │
│                                                        │
│  Update progress tracker (maybe emit every 500ms)     │
│  Update DB indexed count                              │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────────┐
│ 7. Wait for Thumbnail Background Tasks                  │
│    - JoinSet collects all spawn results                │
│    - Final progress update                             │
│    - Clean up in-progress tasks on cancellation        │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────────┐
│ 8. Emit index_finished event                            │
│    - Final stats: indexed, processed, search_ready,    │
│      total, failed counts                              │
│    - Timing information                                │
│    - Mark album fully indexed in DB                    │
└──────────────────────────────────────────────────────────┘
```

### Core Algorithms

#### Progressive Indexing with Resume

**Scenario:** User clicks Index → Indexing crashes at file 50/100 → App restarts

**Recovery Process:**
1. On next index request, compare counts:
   - `previous_total` = 100 (from DB)
   - `current_total` = 100 (from new scan)
   - `previous_indexed` = 50 (incomplete state)
   - **Condition:** If all three are consistent, `resume_from = 50`

2. During traversal, skip first 50 files (via `traversed_count`)

3. Resume from file 51, maintaining DB integrity

**Safety:** Index trace written to file for debugging post-crash diagnostic

#### Single File Indexing (`index_single_file`)

```
Input: file_path, album_id, file_type, thumbnail_size, scan_time
Output: IndexOutcome {
  task: Option<ThumbnailTask>,
  processed_immediately: bool,
  search_ready_immediately: bool
}

Steps:
  1. Read file metadata (size, modification date)
  2. If file is media (not sidecar):
     a. Extract EXIF using exif library or VideoInfo
     b. Calculate BLAKE3 hash on file contents
     c. Determine if hash is duplicate (dedup detection)
     d. Extract metadata: dimensions, duration, capture_time, camera, lens, GPS
     e. Insert into afiles table (SQL INSERT OR REPLACE)
     f. Create ThumbnailTask if file is searchable (image/video)
     g. Mark as processed_immediately (metadata only)
     h. Mark as search_ready_immediately if no thumbnail needed
     i. Return IndexOutcome with optional ThumbnailTask
  3. If file is sidecar (.AAE, .XMP, etc.):
     - Skip (handled by Apple sidecar manager)
     - Return None (no task)
```

#### File Type Detection

```
Input: file_path
Output: FileType { 0=Image, 1=RAW, 2=Video } or None

Logic:
  1. Extract extension (case-insensitive)
  2. Check in format lists (priority order):
     - t_common::RAW_IMGS       → return 1
     - t_common::VIDEOS         → return 2
     - t_common::NORMAL_IMGS    → return 0
     - t_common::FFMPEG_BACKED  → return 0
  3. If not found, return None (skip file)
```

#### Metadata Extraction

**EXIF-based (Images/RAW):**
- Use `kamadak-exif` library to parse EXIF tags
- Extract: DateTimeOriginal, Make, Model, LensModel, FocalLength, ISO, FNumber, ExposureTime
- Fallback to file modification time if DateTimeOriginal missing
- Extract GPS: GPSLatitude, GPSLongitude, GPSAltitude

**Video Metadata:**
- Use FFprobe (via FFmpeg sidecar) to extract duration, codec, dimensions
- Extract creation date from container metadata

**Hash Calculation:**
- BLAKE3, streamed in 64 KB chunks (flat memory)
- Computed only for files whose `size` is shared with another file (a unique size cannot be an exact duplicate) and whose stored `mtime` differs from the file's `modified_at`
- Stored in `file_hashes`, keyed by `file_id`; `hash` is intentionally non-unique so copies group together in `duplicate_groups`

### Thumbnail Generation Pipeline

#### Async Task-Based Design

**Why async?**
- Main indexing loop continues discovering files
- Thumbnails generated in parallel (up to 4 concurrent by default)
- Backpressure: if too many tasks queue, loop yields

**Architecture:**
```
┌──────────────────┐
│ Main Loop        │
│ (WalkDir)        │
└────────┬─────────┘
         │
         ├─→ Spawn ThumbnailTask
         │   (into JoinSet)
         │
         ├─→ Spawn ThumbnailTask
         │   (into JoinSet)
         │
         └─→ Continue traversal...
                   │
                   ▼
         ┌────────────────────┐
         │ Thumbnail Worker   │
         │ (Tokio task)       │
         ├────────────────────┤
         │ 1. Acquire semaphore│
         │ 2. Decode format   │
         │ 3. Resize to thumb │
         │ 4. Store in DB     │
         │ 5. Release sem     │
         └────────────────────┘
```

#### Format-Specific Decoders

**JPEG:** `t_jpeg.rs`
- Use libjpeg-turbo (fast C library)
- In-place downsampling during decode
- Preserves orientation from EXIF

**PNG:** `t_image.rs`
- Pure Rust image crate decoder
- Simple resize via fast_image_resize

**RAW:** `t_libraw.rs`
- LibRaw C binding through FFI
- Extract embedded thumbnail if available
- Full demosaic if needed

**HEIC/HEIF:** `t_heif.rs` (non-macOS)
- libheif C binding
- Native macOS support via native APIs

**Video:** `t_video.rs`
- FFmpeg sidecar extracts keyframe
- Run via FFmpeg subprocess
- Fallback to placeholder if extraction fails

**JXL:** `t_jxl.rs`
- jxl-oxide Rust library
- Pure Rust, no external dependencies

#### Concurrency Control

**Semaphore Limit:**
```rust
let semaphore = Arc::new(Semaphore::new(4)); // Max 4 concurrent
for task in tasks {
    let permit = semaphore.acquire().await?;
    // Generate thumbnail
    drop(permit); // Release slot
}
```

**Benefits:**
- Prevents memory exhaustion with large folders
- Balances main loop speed vs. resource usage
- User-configurable if needed

### Performance Characteristics

#### Time Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Initial index (n files) | O(n) | Linear WalkDir traversal |
| EXIF extraction | O(1) | Per-file constant time |
| Hash calculation | O(file_size) per candidate | Only same-size candidates are read; unchanged files skipped by mtime |
| Thumbnail gen | O(w×h) | Proportional to image dimensions |
| Re-scan (unchanged) | O(n) | Still traverses all, but resume available |

#### Space Complexity

| Component | Space | Notes |
|-----------|-------|-------|
| Database | O(n) | ~1-2KB per file (metadata) |
| Thumbnails cache | O(n × thumb_size) | ~50-100KB per thumbnail |
| Progress tracker | O(1) | Fixed-size stats struct |
| AI embeddings | O(search_files × dim) | 512 dims × 4 bytes = 2KB per image |
| Face embeddings | O(faces × 128) | 128 dims × 4 bytes = 512B per face |

#### Empirical Performance

**Test: 10,000 mixed images/RAW/videos** (measured at 10k only; larger sizes are unmeasured, see DESIGN_ISSUES_AND_CLARIFICATIONS.md)
- Traversal + metadata extraction: ~5-10 seconds
- Thumbnail generation (4 concurrent): ~20-30 seconds
- Total first index: ~30-40 seconds
- Re-scan (unchanged files): ~5-10 seconds (traversal only)
- Resume from crash: ~1-2 seconds (skip 50%, continue from point)

### Error Recovery

#### Crash Recovery via Index Trace

**Mechanism:**
- Before processing each file, write `{album_id, file_path}` to trace file
- On app crash, trace file remains on disk
- Next index detects incomplete state

**Retrieval:**
```
get_index_recovery_info() → Option<IndexRecoveryInfo>
├─ album_id: i64
├─ last_file_path: String
└─ timestamp: i64
```

**UI Action:**
- Show recovery dialog to user
- Option: "Resume" → Continue from last_file_path
- Option: "Skip & Continue" → Skip problematic file
- Option: "Restart" → Begin fresh index

#### Error Handling Strategies

| Error Type | Handling | Result |
|-----------|----------|--------|
| Corrupted image | Catch panic, log path, continue | Mark as failed, skip thumbnail |
| Missing sidecar | Warn in log, continue | Proceed with file (parent handled) |
| Out of disk space | Stop indexing, emit error | User must free space to retry |
| Permission denied | Skip directory, log path | Continue to next directory |
| File deleted mid-scan | File already removed, skip | No DB change needed |
| DB locked | Retry with backoff | Resume when DB available |

### Incremental Indexing Strategy

#### First Run (Cold Start)
- `resume_from = 0`
- All files processed
- Thumbnails generated for all
- Full AI embedding extraction

#### Subsequent Runs (Warm Start)
- **New files:** Detected by traversal, indexed normally
- **Modified files:** File's modification time checked against `indexed_at`
  - If `file_modified_at > indexed_at`: Re-process
  - Else: Skip (already indexed)
- **Deleted files:** Detected via soft-delete or no longer exists
  - Marked as `deleted_at` or removed from DB
  - Thumbnails optionally cleaned up
- **Moved files:** 
  - If only path changed, folder migration handles via inode
  - Database entry updated, metadata preserved

#### Resume Logic

```
Trigger: Index crash detected

1. Load last_file_path from trace
2. Count current total files
3. Load previous total from DB

Decision:
  if previous_total == current_total:
    resume_from = DB.indexed_count (skip to here)
  else:
    resume_from = 0 (totals changed, full re-scan)

Benefit:
  - No data loss on crash
  - Avoids duplicate work
  - User-transparent recovery
```

**📚 For a detailed quick-reference summary of this section, see [INDEXING_ENGINE_ANALYSIS.md](./INDEXING_ENGINE_ANALYSIS.md)** — includes component tables, algorithm walkthroughs, performance metrics, code references, and key takeaways.

---

## Media Processing Pipeline

### Image Indexing & Thumbnail Generation

**Flow:**
1. Scan album folder for new/modified files
2. For each file:
   - Extract EXIF metadata (dimensions, capture date, camera, GPS, etc.)
   - Calculate file hash for deduplication
   - Determine file type (Image/RAW/Video)
   - Insert file record into database
3. Generate thumbnail (async background task)
4. Extract AI embedding (if search enabled)
5. Mark album as indexed

**Thumbnail Generation:**
- **JPEG:** Use libjpeg-turbo for fast decoding and resizing
- **PNG:** Decoded via image crate
- **HEIC/HEIF:** libheif binding (for non-macOS; native on macOS)
- **RAW:** LibRaw decoding → preview generation
- **Video:** FFmpeg extraction of keyframe
- **JXL:** jxl-oxide library

Thumbnails are cached in Lap's application data directory and keyed by file hash + size.

### File Format Support

**Standard Images:**
- JPEG/JPEG2000, PNG, GIF, BMP, TIFF, WebP, HEIC/HEIF, AVIF, JXL

**RAW Formats:** (60+ camera models)
- Canon (CR2, CR3, CRW)
- Nikon (NEF, NRW)
- Sony (ARW, SRF, SR2)
- Fujifilm (RAF), Panasonic (RW2), Olympus (ORF), Pentax (PEF)
- Generic (DNG, RAW)

**Video:**
- MP4, MOV, MKV, AVI, WebM, FLV, WMV, 3GP, HEVC, TS/M2TS (40+ codecs via FFmpeg)

---

## AI & ML Pipeline

### Image Search (CLIP-based)

**Vision Model:** ResNet50-based CLIP vision encoder
**Text Model:** Transformer tokenizer + CLIP text encoder

**Indexing Process:**
1. Load full-resolution image
2. Normalize to 224×224 RGB
3. Run through vision model → 512-dim embedding vector
4. Store embedding in AI embeddings table

**Query Process:**
1. User enters text query
2. Tokenize text using BERT tokenizer
3. Run through text model → 512-dim embedding
4. Compute cosine distance to all indexed embeddings
5. Return top-K results sorted by similarity

**Multilingual Support:**
- Download multilingual text model on demand
- Same vision model; only text encoder changes
- User can switch between default (English) and multilingual (50+ languages)

**Models:**
- Located in `src-tauri/resources/models/`
- Text: `text_model.onnx` (200MB)
- Vision: `vision_model.onnx` (100MB)
- Tokenizer: `tokenizer.json` (vocabulary for text encoding)

### Face Recognition Pipeline

**Components:**
- **Detection Model:** RetinaFace (500M variant) → Detects face bounding boxes + 5 landmarks
- **Embedding Model:** MobileFaceNet → Generates 128-dim face embedding per face

**Face Indexing:**
1. For each file in album:
   - Load image
   - Run RetinaFace detection → Bounding boxes
   - Filter by confidence (>0.65) and blur score
   - For each valid face:
     - Align face to canonical pose using landmarks
     - Run MobileFaceNet → 128-dim embedding
     - Store face record with embedding
2. After detection:
   - Run hierarchical clustering on embeddings
   - Build HNSW graph for fast similarity search
   - Group faces into clusters (people)
3. User can refine clusters manually

**Face Query:**
- Given a person cluster, find all faces in that cluster
- Retrieve all associated files
- Sort by face quality (inverse blur score)

**Models:**
- Detection: `det_500m.onnx` (10MB, 500k params)
- Embedding: `w600k_mbf.onnx` (25MB, MobileFaceNet)

### Similarity Search

**Text / image-by-example search:** cosine similarity over `afiles.embeds`, computed by a linear scan in Rust (O(N) per query; no ANN index). Thresholds come from settings (text search presets 0.255-0.32; "find related photos" 0.7).

**Visual near-duplicate grouping** (`t_similar.rs`):
- CLIP embeddings indexed in an in-memory HNSW graph (cosine distance; M=16, ef_construction=200, search ef=200, top 63 neighbours)
- Pairs at or above the user-selected threshold (presets 0.97 / 0.93 / 0.90 / 0.85, default 0.93) become edges
- Complete-linkage clustering: two clusters merge only if every cross pair meets the threshold; groups are capped at 64 files
- Exact duplicates use BLAKE3 (see Hash Calculation), not this path; no perceptual hash is implemented

**Configuration:**
- Min confidence threshold: 0.65
- Min blur score: 200.0 (Laplacian variance)
- Face size ratio ignored; use absolute pixel thresholds

---

## File Operations & Safety

### Safe Move/Copy/Delete

Lap maintains referential integrity when files are reorganized:

**Key Principles:**
1. All file operations update the database in sync with filesystem
2. Grouped assets (Apple Live Photos, RAW+JPEG pairs) are kept together
3. Metadata (tags, collections, ratings) follow files

**Apple Sidecar Preservation:**
- Apple Live Photos: MOV video + HEIC JPEG + `.AAE` sidecar
- RAW+JPEG pairs: `IMG_123.CR2` + `IMG_123.JPG` (same basename)
- When renaming/moving:
  - Rename all components atomically
  - Update all file records in DB
  - Verify no orphaned sidecars left behind

**Rename Operation:**
1. Build rename plan (all components)
2. Pre-flight checks (no conflicts, adequate permissions)
3. Execute filesystem renames
4. Update DB file records
5. Rollback on any step failure

### Thumbnail Locking

Concurrent thumbnail generation is protected by:
- Per-file hash lock (ThumbGenerationLocks)
- Wait mechanism to prevent duplicate generation
- Background task tracking

---

## Configuration Management

### App Configuration (`app-config.json`)

Located in app data directory; persists:
- List of libraries (albums)
- Current selected library
- Per-library state (UI selections, panel visibility)
- Smart Album definitions
- User preferences

**Multi-Library Support:**
- Each library has independent database (optional custom path)
- Library switching preserves UI state
- Collections/tags are library-scoped

### Database Location

**Default:**
- macOS: `~/Library/Application Support/com.julyx10.lap/`
- Windows: `%LOCALAPPDATA%\com.julyx10.lap\`
- Linux: `~/.local/share/com.julyx10.lap/`

**Custom Path:**
- User can configure in Settings
- Lap creates/manages database at that location
- Backup recommended for multi-library setups

---

## Performance Optimization

### Indexing Optimization
- **Batch Processing:** Index 100 files at a time before yielding
- **Thumbnail Background Tasks:** Non-blocking async generation
- **Progressive UI Updates:** Emit progress events every 50 files
- **Cancellation Support:** User can stop indexing mid-process

### Query Optimization
- **SQLite Indexes:** On foreign keys (`file_id`, `folder_id`, `album_id`), `file_hashes(hash, file_size)`, and others defined in `t_sqlite.rs` / `t_migration.rs`
- **Pagination:** Fetch 100 items per page by default
- **Lazy Loading:** Load metadata on-demand, not all at startup

### Memory Management
- **Image Downsampling:** Load web-sized images, not full resolution
- **Embedding Cache:** Pre-computed and stored in DB
- **Streaming Video:** Use FFmpeg sidecar, not in-process

### Threading Model
- **Main Thread:** Tauri event loop (IPC, UI updates)
- **Tokio Runtime:** Async I/O, background tasks
- **Worker Threads:** Image processing, face detection (parallel per-file)
- **Rayon Parallelism:** Batch operations (copy, delete, dedup)

---

## Privacy & Security

### Design Principles
1. **Local-First:** All data stays on user's machine
2. **No Cloud:** No required account, login, or upload
3. **Transparent Indexing:** User controls when indexing happens
4. **No Telemetry:** Optional analytics (Aptabase) disabled by default

### Data Isolation
- Database: SQLite file on local disk (no encryption by default)
- AI Models: Bundled; run entirely locally on ONNX Runtime
- Thumbnails: Cached locally, keyed by file hash
- Metadata: Never left unencrypted on disk (but not encrypted by default; users should back up according to their security needs)

### File Permissions
- Lap respects OS file permissions
- Deleted files moved to OS trash, not permanently deleted
- No privileged access required

---

## Error Handling & Logging

### Error Types

**Database Errors:**
- File corrupted/locked → Return user-friendly error, suggest database rebuild
- Schema mismatch → Run migration
- Out of disk space → Alert user

**Media Processing Errors:**
- Unsupported format → Skip file, log warning
- Corrupted image → Return placeholder, continue indexing
- Missing sidecar → Warn user, continue

**AI Model Errors:**
- Model not found → Return error, suggest re-download
- Out of memory → Reduce batch size, retry
- Inference failure → Log error, skip that file

### Logging
- Rust `eprintln!` for critical errors (desktop output visible in debug builds)
- Optional structured logging for diagnostics
- No PII logged (file paths sanitized in production)

---

## Extension Points

### Custom Format Support
- Add decoder in appropriate module (`t_image.rs`, `t_libraw.rs`, etc.)
- Register extension in `t_common.rs` format lists
- Update file type detection logic

### Custom Metadata Extraction
- EXIF → Add columns to `afiles` in `t_sqlite.rs` (plus a migration in `t_migration.rs`)
- GPS → Add Leaflet map integration in frontend
- Custom XMP → Parse and store in extended metadata

### Additional AI Models
- Load additional ONNX models in `t_ai.rs`
- Create new embedding tables for different model versions
- Implement search interface

---

## Deployment & Building

### Build Requirements
- Node.js 20+, pnpm
- Rust stable + Tauri CLI
- System dependencies: NASM, pkg-config, autoconf, automake, libtool, cmake
- Platform-specific: Xcode (macOS), Visual Studio Build Tools (Windows)

### Build Process
1. Download AI models and FFmpeg sidecar
2. Install npm dependencies (`src-vite/`)
3. Compile Rust backend
4. Bundle Vite frontend into Tauri app
5. Package platform-specific installer

### Release Artifacts
- **macOS:** `.dmg` (notarized; universal binary for Intel/Apple Silicon)
- **Windows:** `.msi` (x64/ARM64 variants)
- **Linux:** `.deb` (Debian-based; amd64/arm64)

---

## Future Enhancements

### Planned Features
- **Video Metadata:** Full frame extraction for keyframe selection
- **Search Refinement:** Learned user preferences for ranking
- **Collaborative Collections:** Share collections between libraries (local network)
- **WebRTC Playback:** Eliminate FFmpeg dependency for some codecs
- **GPU Acceleration:** Optional CUDA/Metal for AI inference

### Potential Optimizations
- **Incremental Face Clustering:** Update clusters without full re-indexing
- **Compression:** Store thumbnails in WebP for smaller cache
- **Shader-Based Thumbnails:** GPU-accelerated resizing
- **Parallel Indexing:** Multi-threaded scanning across albums

---

## References

- **Main Entry:** [main.rs](D:\Github\lap\src-tauri\src\main.rs)
- **Database:** [t_sqlite.rs](D:\Github\lap\src-tauri\src\t_sqlite.rs)
- **Commands:** [t_cmds.rs](D:\Github\lap\src-tauri\src\t_cmds.rs)
- **AI Engine:** [t_ai.rs](D:\Github\lap\src-tauri\src\t_ai.rs)
- **Face Recognition:** [t_face.rs](D:\Github\lap\src-tauri\src\t_face.rs)
- **Configuration:** [t_config.rs](D:\Github\lap\src-tauri\src\t_config.rs)
- **Common Constants:** [t_common.rs](D:\Github\lap\src-tauri\src\t_common.rs)

---

## 📋 Documentation & Analysis Resources

This design document is supplemented by companion analysis documents:

- **[INDEXING_ENGINE_ANALYSIS.md](./INDEXING_ENGINE_ANALYSIS.md)** — Quick-reference summary of the indexing engine architecture, algorithms, performance metrics, and code locations.

- **[DESIGN_ISSUES_AND_CLARIFICATIONS.md](./DESIGN_ISSUES_AND_CLARIFICATIONS.md)** — Critical design review addressing architectural questions, performance considerations, and documentation gaps. Includes analysis of:
  - Duplicate detection (`file_hashes` handling)
  - Performance scaling to large libraries (1TB+)
  - Near-duplicate detection via visual similarity
  - Vector search architecture (brute-force cosine scan vs. ANN)
  - Table naming conventions

---

**Document Version:** 1.0  
**Last Updated:** 2026-11-18 (schema and similarity sections corrected against source)  
**Project:** Lap v0.3.0

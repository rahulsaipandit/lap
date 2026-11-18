# Blurry Photo Finder + PromptFix Auto-Fix — Design Notes

## User Workflow

1. **Open the folder** in `lap` (Album/Folder view) as normal.
2. **Click the "Find blurry photos" button** in the content toolbar (next to the existing
   dedup buttons) — this opens a right-side panel scoped to whatever folder/query you're
   currently viewing.
3. The panel **auto-scans** that view for blur (Laplacian-variance per photo — the same
   metric already used internally for face quality) and lists every photo below the
   threshold, blurriest first, with a thumbnail and its blur score.
4. **Selection**: each row has a checkbox; a "Select all" toggle sits at the top of the
   panel. Single-click a row selects it for viewing (emits to the main viewer),
   double-click opens it in the media viewer, the checkbox toggles it into the fix batch.
5. **Fix**: once one, many, or all photos are checked, click **"Fix N"** — this sends
   each selected photo to the PromptFix FastAPI service (started manually beforehand) and
   writes back a `<name>_fixed.<ext>` file next to the original once done; the panel shows
   a "Fixed" badge on completed items.

## Context

`lap` already computes a blur score (Variance-of-Laplacian) internally in
[t_face.rs](../../src-tauri/src/t_face.rs), but only per detected face crop, and only to
silently discard low-quality faces before embedding — it's never persisted or surfaced to
the user. This feature makes blur detection first-class and user-visible, and adds an
optional batch "fix" action powered by [PromptFix](https://github.com/yeates/PromptFix), a
diffusion-based image restoration model, wrapped in a small standalone FastAPI service that
the user starts manually alongside `lap`.

### Confirmed decisions

- **License**: PromptFix is research/non-commercial only — acceptable since this is
  personal use, not distributed commercially.
- **Deployment**: the FastAPI service is **manually started** (not a Tauri-managed
  sidecar) — Rust just makes HTTP calls to `http://127.0.0.1:PORT` and surfaces a clear
  error if it's unreachable.
- **Scope**: the feature scans for **all** blurry photos in the current view and offers a
  **batch fix**, not single-photo-only.

## Research Findings

### PromptFix inference API

- **Entry point**: `scripts/inference.sh` downloads the checkpoint via `wget` then runs
  `python process_images_json.py` (repo root) — there is no per-call CLI or importable
  `run(image_path, instruction)` function. `process_images_json.py` is an
  InstructPix2Pix-style batch tool (flags: `--resolution 640`, `--steps 20`,
  `--config configs/promptfix.yaml`, `--ckpt ./checkpoints/promptfix.ckpt`, `--indir`,
  `--outdir`, `--cfg-text 6.5`, `--cfg-image 1.25`, `--seed 2024`). It loads a directory of
  images plus a sibling `instructions.json` (filename → `[instruction, flaw_prompt]`),
  loops over all images, and writes JPEGs to `--outdir`. Integrating it meant refactoring
  its model-loading + single-sample denoising loop into a reusable, load-once class instead
  of shelling out per image.
- **Inputs**: an RGB PIL image, normalized to `[-1, 1]`, resized to a 64px-aligned
  resolution (default 640). No deblur example ships in `examples/validation/instructions.json`
  (only colorize/dehaze/desnow/dewatermark/lowlight/inpaint), but by the pattern used for
  analogous tasks, an instruction like **"Remove the blur from the image."** is a
  reasonable fit — per the paper, GPT-4 generated diverse phrasings per task category, so
  exact wording isn't rigid.
- **Dependencies**: `requirements.txt` lists taming-transformers (git), `kornia==0.6`,
  k-diffusion (git), CLIP (git), deepspeed, termcolor, timm, easydict, omegaconf,
  opencv-python, transformers, pyarrow, `pytorch-lightning==1.9.0`, pandas. No pinned
  torch/torchvision/CUDA version anywhere — install is `conda create -n promptfix
  python=3.10` + `pip install -r requirements.txt`, with torch/CUDA installed separately.
  Given the Stable-Diffusion/InstructPix2Pix-derived architecture (k-diffusion sampler,
  classifier-free guidance, 20-step denoising) plus `deepspeed`, this is GPU-oriented; CPU
  inference is technically possible but likely very slow, and `deepspeed` on non-CUDA/Windows
  hosts is a real risk.
- **Weights**: auto-downloaded from Hugging Face
  (`https://huggingface.co/yeates/PromptFix/resolve/main/promptfix.ckpt` →
  `checkpoints/promptfix.ckpt`), size undisclosed but likely several GB (SD1.5-based UNet +
  VAE + CLIP).
- **Latency**: no concrete numbers published anywhere (README, GitHub, arXiv abstract
  2405.16785) — plan the batch UX around unknown per-image latency rather than a documented
  figure.

### `lap`'s scan lifecycle pattern (`t_similar.rs`)

- **State**: `SimilarState { is_scanning: Arc<AtomicBool>, cancel_flag: Arc<AtomicBool>,
  status: Arc<Mutex<SimilarScanStatus>> }`, registered as Tauri managed state.
  `SimilarScanStatus` is a `Serialize` struct (camelCase) with `state`, `phase`,
  `scope_key`, `current`, `total`, `groups`, `is_scanning`.
- **`start_scan`**: guards against concurrent runs via `is_scanning.compare_exchange`,
  resets `cancel_flag`, sets initial status to `running`/`preparing`, clones the
  `status`/`is_scanning`/`cancel_flag` `Arc`s, then `std::thread::spawn` runs `scan(...)`.
  On completion it sets the final state (`finished`/`idle` if cancelled/`error`), flips
  `is_scanning` false, and emits a final `similar-scan-progress` event.
- **Progress events**: single event name `similar-scan-progress`, payload is
  `SimilarScanStatus`, emitted at scope resolution, periodically during matching, and once
  at the end.
- **Cancellation**: cooperative — `cancel_flag: Arc<AtomicBool>` checked at loop
  checkpoints; `cancel_scan()` just stores `true`. Cancelled scans return early without
  writing to the DB (results are only written once fully computed, in a single
  transaction).
- **SQLite writes**: opens a `conn.transaction()`, deletes prior rows for the same scope,
  inserts one `similarity_scans` row, then per-cluster rows into `similarity_groups` and
  `similarity_group_items`, commits once. `get_db_conn()` opens a fresh `Connection` via
  `t_storage::get_current_db_path()`.
- **Read APIs**: `list_groups`, `get_group`, `has_scan` — all query by `scope_key` join.

### HTTP client availability

`reqwest` is already a dependency (`Cargo.toml`: `reqwest = { version = "0.12.28",
default-features = false, features = ["rustls-tls"] }`, plus `tokio = { features = ["full"]
}`). Existing usage: `t_ai.rs` (async `reqwest::Client` for model downloads), `t_cmds.rs`
(`reqwest::get(url)`). No new crate needed — the `multipart` feature was added for
uploading images to the FastAPI service.

### Existing image-write pattern

- `t_image.rs` `fs::write(output_path, data)` — writes processed image bytes to an
  arbitrary output path.
- `t_image.rs` `write_jpeg_exif_block` — reads dest file bytes, patches EXIF,
  `fs::write(dest_path, output)` — read-modify-write to an existing path.
- No `_edited`/`_fixed` suffix convention existed previously — introduced a
  `<name>_fixed.<ext>` sibling-file convention for this feature.

### Migration pattern (`t_migration.rs`)

Migrations are a `Vec<Migration>` (`Migration { version: i32, description: &'static str,
sql: &'static str }`). Version 10 is the template used for `blur_scans`/`blur_results`: a
multi-statement `sql` string creating a parent scan table (`scope_key TEXT UNIQUE`,
status/counts/timestamps) and a results table with FKs to both the scan and `afiles(id)`
(`ON DELETE CASCADE`), plus indexes.

### `AFile` struct / `afiles` columns (`t_sqlite.rs`)

Key fields for joining: `id: Option<i64>` (PK), `folder_id: i64`, `name: String`,
`file_type: Option<i64>` (1 = image, 2 = video, 3 = audio, 4 = other), `width`/`height`,
`taken_date`, `modified_at`, plus output-only `file_path: Option<String>` (computed for the
webview via `AFile::get_file_info`/`build_base_query`, not a raw column). `blur_results`
resolves each file's disk path at read-time via `AFile::get_file_info(file_id)` rather than
storing a path column, matching the existing convention.

### App menu / UI architecture

- `t_menu.rs` is a native, macOS-only app menu (About/Preferences/Help) — it has no
  File/Photo/Tools submenu and no per-photo actions.
- The actual user-facing "menu items" for photo actions are frontend context-menu entries
  (`src-vite/src/common/fileMenu.ts`), dispatched as string action names to a handler map
  in `Content.vue`.
- The toolbar-button + right-panel pattern (not the native OS menu) is what "Find blurry
  photos" follows — same as the existing Dedup/Similar buttons in `Content.vue`, which
  toggle `config.rightPanel.mode` and conditionally render a panel component
  (`DedupPane.vue`, `DedupPane_2.vue`, and now `BlurryPane.vue`).

## PromptFix Evaluation Summary

PromptFix is a diffusion-model-based image restoration tool that follows human
instructions to fix degraded photos (colorization, object removal, dehazing, deblurring,
watermark removal, snow removal, low-light enhancement) via a 20-step denoising process,
built on Stable Diffusion / InstructDiffusion. It is Python/PyTorch only, with no Rust API,
CLI binary, or HTTP service of its own — hence the need for the `promptfix_service`
FastAPI wrapper in this repo. Its license (Apache 2.0 code, but explicitly research/
non-commercial use and a dataset with inherited licensing restrictions) is acceptable only
because this integration is for personal, non-commercial use of `lap`.

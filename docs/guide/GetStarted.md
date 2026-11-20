# Getting Started with Lap (this fork)

A practical guide to running, building and managing data for this fork of
[julyx10/lap](https://github.com/julyx10/lap). Commands are for Windows PowerShell.

**On this page**

1. [Run the app in dev mode](#1-run-the-app-in-dev-mode)
2. [Add your photo folders](#2-add-your-photo-folders)
3. [Resume a paused scan](#3-resume-a-paused-scan)
4. [Where your data lives](#4-where-your-data-lives)
5. [Dev build vs. production build](#5-dev-build-vs-production-build)
6. [Build an installer](#6-build-an-installer)
7. [Troubleshooting](#7-troubleshooting)

---

## 1. Run the app in dev mode

Open a **new** terminal (so `cargo` is on your PATH), then:

```powershell
cd d:\Github\lap\src-vite
pnpm install          # first time, or after dependencies change
cd ..
cargo tauri dev
```

`cargo tauri dev` starts the Vite dev server (`localhost:3580`) and the app together.
The first Rust compile takes about 10 minutes; later runs are incremental.

If `cargo tauri` is not found: `cargo install tauri-cli --version "^2.0.0" --locked`.

If the app is missing AI models or FFmpeg, run `.\scripts\download_models.ps1` and
`.\scripts\download_ffmpeg_sidecar.ps1`.

### Desktop shortcut: "Lap (dev)"

The debug `Lap.exe` (`src-tauri\target\debug\Lap.exe`) loads its UI from the Vite dev server,
so it cannot be started on its own. The **Lap (dev)** shortcut runs a launcher
(`%LOCALAPPDATA%\lap-dev-launcher\launch.ps1`) that:

1. Starts Vite if nothing is listening on port 3580 (it checks both IPv4 and IPv6).
2. Starts `Lap.exe`.
3. Stops the Vite server it started when you close the app.

Errors show in a message box, and the launcher writes `launch.log`, `vite.out.log` and
`vite.err.log` next to the script.

Notes:

- `Lap.exe` is a snapshot of the last Rust build. After changing Rust code, rebuild with
  `cargo tauri dev` or `cargo build`. Frontend changes appear through the dev server.
- Don't run two copies of the app at once. They share one SQLite database.

---

## 2. Add your photo folders

Lap is folder-first: it never moves or copies your files. To see your existing
Year/date folder structure:

1. Open **Albums** in the left sidebar.
2. Click the **+** button and pick your top-level photo folder (for example the folder that
   holds your year folders).
3. Confirm. Lap indexes it, and the album expands into a folder tree that mirrors the disk.

Related options:

- **Show files from subfolders** controls whether opening a folder also shows its child folders.
- The album dialog lets you exclude folders and filter file types and small images.
- **Import & Organize** is a separate, optional feature that *copies* files into date-based
  layouts. You don't need it to browse an existing structure.

---

## 3. Resume a paused scan

"Scanning paused" appears when an album's scan stopped before finishing. The status-bar
message is not clickable. To resume:

1. Open **Albums** in the left panel.
2. Find the album. Its icon is a circular arrow with a dot, tooltip "Scan paused".
3. Click the icon. The scan restarts and continues where it stopped, as long as the file
   count hasn't changed.

While it runs the icon spins and the status bar shows `Scanning… count/total`.
Clicking the spinning icon cancels the scan.

If nothing happens:

- The album's folder may be unreachable (the icon is dimmed). Check the drive or path.
- After a crash you may see a **Scanning interrupted** dialog. Continue, and tick
  **skip this file** if one file keeps causing the interruption.

---

## 4. Where your data lives

Lap keeps three things apart, and the **config file decides where the other two are**:

| What | Default location | Notes |
|---|---|---|
| Config (`app-config.json`) | `%LOCALAPPDATA%\com.julyx10.lap.debug\` (dev) or `...\com.julyx10.lap\` (production) | Holds the paths below |
| Database (`default.db`, `-wal`, `-shm`) | `<app folder>\libraries\` | Can be moved with the `db_storage_dir` setting |
| Thumbnails | `<app folder>` cache | Can be moved with the `thumbnail_cache_dir` setting |

**In this setup** both were moved to `D:\Temp\Lap`:

| What | Location | Size when checked |
|---|---|---|
| Database | `D:\Temp\Lap\default.db` (+ `-wal`, `-shm`) | 360 MB + 70 MB WAL |
| Thumbnails | `D:\Temp\Lap\lap-thumbnail-cache` | 48,681 files, 1.8 GB |
| Config | `%LOCALAPPDATA%\com.julyx10.lap.debug\app-config.json` | 2 KB |

The index represents all the scanning work done so far; that is what you'd lose if this data went missing.

Because the app folder (`libraries\`) is empty in this setup, **copying only the app folder does
not back up your index.**

### Back up the index

Close Lap first, then copy the database files **together** (the `-wal` file holds recent writes):

```powershell
New-Item -ItemType Directory D:\LapBackup -Force
Copy-Item D:\Temp\Lap\default.db*, D:\Temp\Lap\lap-thumbnail-cache -Destination D:\LapBackup -Recurse
Copy-Item "$env:LOCALAPPDATA\com.julyx10.lap.debug\app-config.json" D:\LapBackup
```

### Database migrations

The first launch of a newer build migrates the database in place (upstream migrations 15-24
plus this fork's blur-scan migration 25). After that, an older build can't open it, so back up
before running a new build for the first time. The migration unit tests pass, but they have
not been run against a real library. A migration may also trigger re-processing of some files
(thumbnails or metadata); that is a re-scan, not a loss of the index.

---

## 5. Dev build vs. production build

A production build contains the compiled frontend inside the executable (served through
Tauri's internal protocol), so it needs no Vite or Node at runtime. `cargo tauri build` produces
it; the plain debug `Lap.exe` expects the dev server instead.

What differs:

| | Dev / debug build | Production build |
|---|---|---|
| Frontend | Vite dev server (`localhost:3580`) | Embedded in the exe |
| App data folder | `%LOCALAPPDATA%\com.julyx10.lap.debug` | `%LOCALAPPDATA%\com.julyx10.lap` |
| Models and FFmpeg | `src-tauri\resources` | Bundled with the installer |

The folder difference comes from `cfg!(debug_assertions)` in `t_config.rs`. A production build
starts with an empty library because it reads a different config file.

### Keeping your index when you switch to the production exe

1. **Point production at the same data (recommended).** Copy the dev
   `app-config.json` to `%LOCALAPPDATA%\com.julyx10.lap\app-config.json`. It carries
   `db_storage_dir` and `thumbnail_cache_dir`, so the production exe opens the same
   database and thumbnails with no re-index. Don't run dev and production at the same time.
   *(Not tested: this follows from how the code resolves the folders.)*
2. **Build a standalone exe that still uses the dev folder.** `cargo tauri build --debug`
   embeds the frontend (no Vite needed) but stays a debug build, so it keeps the `.debug` folder
   and config.
3. **Keep using the Lap (dev) shortcut.** Nothing changes.

Avoid changing the database location from inside the production app's settings: the storage
code can refuse non-empty destinations, so adopting an existing folder isn't guaranteed.

---

## 6. Build an installer

```powershell
cd d:\Github\lap\src-vite
pnpm install
cd ..
cargo tauri build
```

- Runs `pnpm --dir ../src-vite build` first, then compiles in release mode and bundles.
- Output: `src-tauri\target\release\bundle\` (NSIS `.exe` and/or `.msi`, depending on
  `tauri.conf.json`).
- A release compile takes longer than the debug build.
- Signing and updater settings in `tauri.conf.json` have not been checked and could affect the build.

---

## 7. Troubleshooting

| Symptom | Likely cause and fix |
|---|---|
| Double-clicking **Lap (dev)** does nothing | Check `%LOCALAPPDATA%\lap-dev-launcher\launch.log` and `vite.err.log`. The launcher shows a message box on failure. |
| `Lap.exe` shows a blank page | The Vite dev server isn't running on port 3580. Use the shortcut or `cargo tauri dev`. |
| `cargo: command not found` | Open a new terminal. `%USERPROFILE%\.cargo\bin` is already on your user PATH. |
| Library looks empty after switching builds | You switched between the `.debug` and production folders. See [section 5](#5-dev-build-vs-production-build). |
| Settings shows `D:\Temp\Lap/lap-thumbnail-cache` | Fixed: the preview now uses the path's own separator (display only). |
| `git submodule status` errors about `PromptFix` | `PromptFix` is committed as a gitlink with no `.gitmodules` entry. This will break `git submodule update --init --recursive` for fresh clones. |
| Failing tests in a full `cargo test` | Some `t_storage` and `metadata::files` tests were seen failing on Windows in upstream code this fork doesn't change. Re-run to confirm on your machine; the dedup tests (`cargo test t_dedup`) pass. |

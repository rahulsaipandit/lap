# Getting Started with Lap (this fork)

A practical guide to running, building and managing data for this fork of
[julyx10/lap](https://github.com/julyx10/lap). Commands are for Windows PowerShell.

**On this page**

1. [Local developer build](#1-local-developer-build)
2. [Production build](#2-production-build)
3. [Using the app and managing data](#3-using-the-app-and-managing-data)

---

## 1. Local developer build

Use this while changing code. Nothing here creates an installer, so it is the fast path.

### 1.1 Run in dev mode

Open a **new** terminal (so `cargo` is on your PATH), then:

```powershell
cd src-vite
pnpm install          # first time, or after dependencies change
cd ..
cargo tauri dev
```

`cargo tauri dev` starts the Vite dev server (`localhost:3580`) and the app together.
The first Rust compile takes about 10 minutes; later runs are incremental.

- If `cargo tauri` is not found: `cargo install tauri-cli --version "^2.0.0" --locked`.
- If the app is missing AI models or FFmpeg, run `.\scripts\download_models.ps1` and
  `.\scripts\download_ffmpeg_sidecar.ps1`.

### 1.2 Desktop shortcut: "Lap (dev)"

The debug `Lap.exe` (`src-tauri\target\debug\Lap.exe`) loads its UI from the Vite dev server,
so it cannot be started on its own. The **Lap (dev)** shortcut runs a launcher
(`%LOCALAPPDATA%\lap-dev-launcher\launch.ps1`) that:

1. Starts Vite if nothing is listening on port 3580 (it checks both IPv4 and IPv6).
2. Starts `Lap.exe`.
3. Stops the Vite server it started when you close the app.

Errors show in a message box, and the launcher writes `launch.log`, `vite.out.log` and
`vite.err.log` next to the script.

- `Lap.exe` is a snapshot of the last Rust build. After changing Rust code, rebuild with
  `cargo tauri dev` or `cargo build`. Frontend changes appear through the dev server.
- Don't run two copies of the app at once. They share one SQLite database.

### 1.3 Test a release build locally without making an installer

Sometimes you need the real release exe (embedded frontend, release optimizations) but not
an installer. Skip the bundling step:

```powershell
```

Then close the running app and copy the new exe over the installed one. Models and FFmpeg
are already in the install folder, so only the exe changes. First find where Lap is installed
(this prints the folder of the running app, or the installer's registered location):

```powershell
# Option A: Lap is running, so ask the process
(Get-Process Lap -ErrorAction SilentlyContinue).Path

# Option B: read the install location the NSIS installer registered
Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" |
  Where-Object DisplayName -like "*Lap*" | Select-Object DisplayName, InstallLocation
```

Then copy the exe there (the default per-user NSIS location is `%LOCALAPPDATA%\Lap`):

```powershell
Copy-Item src-tauri\target\release\Lap.exe "<your install folder>\" -Force
```

**Why `--no-bundle`:** the slow part of `cargo tauri build` is not compiling, it is the
bundling step (`makensis`). See [section 2.2](#22-why-the-installer-step-is-slow). `--no-bundle`
skips it entirely, so a code-only change costs one release compile instead of a compile plus
several minutes of compression.

Rebuild the installer only when the bundled resources (`models`, `ffmpeg`) or the installer
config changed.

---

## 2. Production build

Use this to produce the installer you distribute or install on a clean machine.

### 2.1 Build the installer

```powershell
cd d:\Github\lap\src-vite
pnpm install
cd ..
cargo tauri build
```

- Runs `pnpm --dir ../src-vite build` first, then compiles in release mode and bundles.
- Output: `src-tauri\target\release\bundle\`. With `"targets": "all"` in `tauri.conf.json`
  both installers are built: `nsis\Lap_0.4.0_x64-setup.exe` and `msi\Lap_0.4.0_x64_en-US.msi`.
- A release compile takes longer than the debug build.
- Signing and updater settings in `tauri.conf.json` have not been checked and could affect the build.

To install the NSIS result silently on your own machine (the MSI uses `msiexec` instead), first check whether Lap is already
installed and where (same commands as in [1.3](#13-test-a-release-build-locally-without-making-an-installer)):

```powershell
(Get-Process Lap -ErrorAction SilentlyContinue).Path

Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" |
  Where-Object DisplayName -like "*Lap*" | Select-Object DisplayName, InstallLocation
```

Then run the installer. Reuse the existing folder to update in place; on a first install
choose one (the default per-user location is `%LOCALAPPDATA%\Lap`):

```powershell
& "d:\Github\lap\src-tauri\target\release\bundle\nsis\Lap_0.4.0_x64-setup.exe" /S /D=$env:LOCALAPPDATA\Lap
```

`/D=` must be the last argument and the path must not be quoted, even if it contains spaces.

### 2.2 Why the installer step is slow

The step that prints `Running makensis to produce ...setup.exe` can take several minutes.
That is expected, for these reasons:

- **Large bundled resources.** The config packs `resources/models/*` (AI model files) and
  `resources/ffmpeg/*` (FFmpeg binaries) into the installer
  ([tauri.windows.conf.json](../../src-tauri/tauri.windows.conf.json)). Measured on this
  machine: about 165 MB of models plus 193 MB of FFmpeg, giving a 192 MB NSIS installer
  (the MSI is 228 MB).
- **LZMA compression.** Tauri's default NSIS compression is LZMA: a small installer, but
  single-threaded and slow. Model files and binaries are already dense, so they barely shrink
  and most of the time is spent on data that gets almost no smaller.
- **Solid compression.** The whole payload is compressed as one block, which is slower again.
- **Antivirus scanning.** Windows Defender scans the large temp files and the output exe as
  they are written.

### 2.3 Ways to speed it up

| Option | When to use it |
|---|---|
| `cargo tauri build --no-bundle` | You only need the exe, not an installer. See [1.3](#13-test-a-release-build-locally-without-making-an-installer). |
| Set `bundle.windows.nsis.compression` to `"none"` (or `"zlib"`) in `tauri.windows.conf.json` | You need an installer for local testing. It gets larger but the step is much faster (not measured here). Keep `lzma` for the release you distribute. |
| Add a Defender exclusion for `d:\Github\lap\src-tauri\target` | Always helps. Avoids scanning build output. |

### 2.4 Dev build vs. production build

A production build contains the compiled frontend inside the executable (served through
Tauri's internal protocol), so it needs no Vite or Node at runtime. The plain debug `Lap.exe`
expects the dev server instead.

|                   | Dev / debug build                      | Production build                 |
|-------------------|----------------------------------------|----------------------------------|
| Frontend          | Vite dev server (`localhost:3580`)     | Embedded in the exe              |
| App data folder   | `%LOCALAPPDATA%\com.julyx10.lap.debug` | `%LOCALAPPDATA%\com.julyx10.lap` |
| Models and FFmpeg | `src-tauri\resources`                  | Bundled with the installer       |

The folder difference comes from `cfg!(debug_assertions)` in `t_config.rs`. A production build
starts with an empty library because it reads a different config file.

**Keeping your index when you switch to the production exe:**

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

## 3. Using the app and managing data

### 3.1 Add your photo folders

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

### 3.2 Resume a paused scan

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

### 3.3 Where your data lives

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

**Back up the index.** Close Lap first, then copy the database files **together** (the `-wal`
file holds recent writes):

```powershell
New-Item -ItemType Directory D:\LapBackup -Force
Copy-Item D:\Temp\Lap\default.db*, D:\Temp\Lap\lap-thumbnail-cache -Destination D:\LapBackup -Recurse
Copy-Item "$env:LOCALAPPDATA\com.julyx10.lap.debug\app-config.json" D:\LapBackup
```

**Database migrations.** The first launch of a newer build migrates the database in place
(upstream migrations 15-24 plus this fork's blur-scan migration 25). After that, an older build
can't open it, so back up before running a new build for the first time. The migration unit
tests pass, but they have not been run against a real library. A migration may also trigger
re-processing of some files (thumbnails or metadata); that is a re-scan, not a loss of the index.

### 3.4 Troubleshooting

| Symptom | Likely cause and fix |
|---|---|
| Double-clicking **Lap (dev)** does nothing | Check `%LOCALAPPDATA%\lap-dev-launcher\launch.log` and `vite.err.log`. The launcher shows a message box on failure. |
| `Lap.exe` shows a blank page | The Vite dev server isn't running on port 3580. Use the shortcut or `cargo tauri dev`. |
| `cargo: command not found` | Open a new terminal. `%USERPROFILE%\.cargo\bin` is already on your user PATH. |
| `makensis` step takes minutes | Expected. See [2.2](#22-why-the-installer-step-is-slow) and [2.3](#23-ways-to-speed-it-up). |
| Library looks empty after switching builds | You switched between the `.debug` and production folders. See [2.4](#24-dev-build-vs-production-build). |
| Settings shows `D:\Temp\Lap/lap-thumbnail-cache` | Fixed: the preview now uses the path's own separator (display only). |
| `git submodule status` errors about `PromptFix` | `PromptFix` is committed as a gitlink with no `.gitmodules` entry. This will break `git submodule update --init --recursive` for fresh clones. |
| Failing tests in a full `cargo test` | Some `t_storage` and `metadata::files` tests were seen failing on Windows in upstream code this fork doesn't change. Re-run to confirm on your machine; the dedup tests (`cargo test t_dedup`) pass. |

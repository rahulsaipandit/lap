From the repo root in PowerShell (`cargo` isn't on the PATH in this shell, so prefix it as shown):

```powershell
$env:Path += ";$env:USERPROFILE\.cargo\bin"
cd d:\Github\lap\src-vite
pnpm install        # only needed if node_modules is missing or out of date
cd ..
cargo tauri dev
```

`cargo tauri dev` starts the Vite dev server on `localhost:3580` itself, through `beforeDevCommand` in `tauri.conf.json`. The first run recompiles the Rust side, which took about 10 minutes here. If `cargo tauri` isn't found, install it once with `cargo install tauri-cli --version "^2.0.0" --locked`. I couldn't check whether it's installed, because `cargo` wasn't on the PATH in the shell I used.

`src-tauri/resources/` already has `models/` and `ffmpeg/`. If it's missing anything, run `.\scripts\download_models.ps1` and `.\scripts\download_ffmpeg_sidecar.ps1`. Upstream may have added new model files in the merge, so re-run `.\scripts\download_models.ps1` if the AI features fail to load.

To use the blurry-photo "Fix with AI" feature, start the PromptFix service separately. It is not part of the Tauri app. From `promptfix_service/`, with its venv active, run `uvicorn app:app --host 127.0.0.1 --port 8712`, and the app's default URL is `http://127.0.0.1:8712`. The service needs the one-time setup in its README, including multi-GB checkpoints and a CUDA GPU.

One thing to check: `git submodule status` fails with "no submodule mapping found in .gitmodules for path 'PromptFix'". Your commit added `PromptFix` as a gitlink, but there is no matching `.gitmodules` entry. That will break `git submodule update --init --recursive` for anyone cloning the repo. Want me to look into it?
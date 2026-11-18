# promptfix_service

A small FastAPI wrapper around [PromptFix](https://github.com/yeates/PromptFix) so `lap`
can send it blurry photos over HTTP and get deblurred versions back.

This is a **standalone Python project** — it is not built or bundled by Tauri/`src-tauri`.
You start it yourself; `lap`'s "Fix All" action just calls its HTTP API.

> **License note:** PromptFix is research-only and explicitly prohibits commercial use.
> This service is intended for personal, non-commercial use of `lap` only.

> **Performance note:** PromptFix pins no CPU-only inference path or published latency
> numbers. It's a Stable-Diffusion-derived 20-step diffusion model — expect it to need a
> CUDA GPU for reasonable speed. CPU inference will likely work but be very slow.

## One-time setup

No conda required — a plain `venv` works fine (PromptFix's dependencies install via pip;
`torch` is installed separately as a CUDA-enabled wheel, which pip handles just as well as
conda). PromptFix's own README pins **Python 3.10**, but 3.11 works too if that's what you
already have installed — use whichever interpreter is available.

1. **Create the venv here, in `promptfix_service/`**:
   ```
   # Windows PowerShell (use whichever Python you have: -3.10, -3.11, -3.12, ...)
   cd promptfix_service   # this folder
   py -3.11 -m venv .venv
   .venv\Scripts\Activate.ps1

   # macOS/Linux
   cd promptfix_service   # this folder
   python3.11 -m venv .venv
   source .venv/bin/activate
   ```
   Keep this venv activated for every step below — `pip install` targets whichever venv is
   active regardless of your current directory, so it's fine to `cd` elsewhere.

2. **Install a CUDA-enabled torch/torchvision build first**, matched to your GPU. For a
   recent (Blackwell/RTX 50-series) GPU you need a build with `sm_120` kernels, e.g.:
   ```
   pip install torch torchvision --index-url https://download.pytorch.org/whl/cu128
   ```
   For other GPUs, see https://pytorch.org/get-started/locally/ for the right command.

3. **Clone PromptFix** somewhere on disk (does not need to be inside this folder) and
   install its dependencies **except `deepspeed`**:
   ```
   cd ..
   git clone https://github.com/yeates/PromptFix.git
   cd PromptFix
   pip install -r requirements.txt
   ```
   `deepspeed`'s wheel build fails on Windows under pip's isolated build environment
   (it can't see the already-installed torch when checking which ops to pre-compile — a
   known upstream issue, not specific to this project). It's also **not actually needed**:
   `deepspeed` is only imported by PromptFix's `main.py` (the training script);
   `process_images_json.py`, which `promptfix_runner.py` is based on, never imports it. If
   the install above fails on `deepspeed`, remove that one line from `requirements.txt` (or
   install everything else individually) and continue — the deblur service works without it.

4. **Download two checkpoints** — PromptFix's config loads both eagerly, not just
   `promptfix.ckpt`:
   ```
   cd PromptFix
   mkdir checkpoints
   curl -L https://huggingface.co/yeates/PromptFix/resolve/main/promptfix.ckpt -o checkpoints/promptfix.ckpt

   # Base Stable Diffusion v1.5 weights — configs/promptfix.yaml sets ckpt_path to this
   # file and LatentDiffusion.__init__ loads it unconditionally (not just for training),
   # so it's required for inference too, not only scripts/train.sh.
   mkdir checkpoints/stable-diffusion-v1
   curl -L https://huggingface.co/runwayml/stable-diffusion-v1-5/resolve/main/v1-5-pruned-emaonly.ckpt -o checkpoints/stable-diffusion-v1/v1-5-pruned-emaonly.ckpt
   ```
   Both are several GB — expect this to take a while. If the `runwayml` repo is
   unavailable, look for a mirror of `v1-5-pruned-emaonly.ckpt` (e.g. under
   `stable-diffusion-v1-5/stable-diffusion-v1-5` on Hugging Face).

5. **Install this service's additional dependencies**, back in `promptfix_service/`:
   ```
   cd ../promptfix_service
   pip install -r requirements.txt
   ```

## Running

Activate the venv, point `PROMPTFIX_DIR` at your PromptFix clone, then start the server:

```
# Windows PowerShell
.venv\Scripts\Activate.ps1
$env:PROMPTFIX_DIR = "C:\path\to\PromptFix"
uvicorn app:app --host 127.0.0.1 --port 8712

# macOS/Linux
source .venv/bin/activate
PROMPTFIX_DIR=/path/to/PromptFix uvicorn app:app --host 127.0.0.1 --port 8712
```

The model loads once at startup (this can take a while) and stays resident for all
subsequent requests.

In `lap`, set the blur-fix service URL to `http://127.0.0.1:8712` (see the app's
settings for the "PromptFix service URL" field). `lap` will call `/health` before
starting a batch fix, and `/deblur` once per blurry photo.

## API

- `GET /health` → `{"status": "ok", "device": "cuda" | "cpu"}`
- `POST /deblur` — multipart form upload with field `file` (an image). Returns the
  deblurred image as `image/jpeg` bytes.

## Files

- `app.py` — FastAPI app: loads the model once at startup, exposes `/health` and `/deblur`.
- `promptfix_runner.py` — refactor of PromptFix's `process_images_json.py` model-loading
  and single-sample denoising loop into a reusable `PromptFixRunner` class (load once,
  call `deblur(image)` many times), fixed to the deblur instruction
  `"Remove the blur from the image."`.

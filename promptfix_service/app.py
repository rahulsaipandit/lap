"""
FastAPI wrapper exposing PromptFix's edit pipeline over HTTP for lap to call.

Run with:  uvicorn app:app --host 127.0.0.1 --port 8712
See README.md for one-time setup (cloning PromptFix, weights, dependencies).
"""

import io
import os

from fastapi import FastAPI, File, Form, HTTPException, UploadFile
from fastapi.concurrency import run_in_threadpool
from fastapi.responses import Response
from PIL import Image

from promptfix_runner import PromptFixRunner, TASK_INSTRUCTIONS

PROMPTFIX_DIR = os.environ.get("PROMPTFIX_DIR", "./PromptFix")

app = FastAPI(title="PromptFix edit service")
runner: PromptFixRunner | None = None


@app.on_event("startup")
def load_model():
    global runner
    runner = PromptFixRunner(promptfix_dir=PROMPTFIX_DIR)


@app.get("/health")
def health():
    if runner is None:
        raise HTTPException(status_code=503, detail="Model not loaded yet")
    return {"status": "ok", "device": runner.device, "tasks": list(TASK_INSTRUCTIONS.keys())}


@app.post("/fix")
async def fix(
    file: UploadFile = File(...),
    task: str | None = Form(default=None),
    instruction: str | None = Form(default=None),
):
    """
    Apply a PromptFix edit to an uploaded image. Either:
    - `task`: one of TASK_INSTRUCTIONS' keys (colorize/dehaze/deblur/dewatermark/desnow/
      lowlight) — uses that task's canonical instruction, or
    - `instruction`: any free-text edit instruction (required for object removal, since
      it has no fixed instruction — e.g. "Extract the boy from the image.").
    Exactly one of `task`/`instruction` should be set; `task` takes precedence if both are.
    """
    if runner is None:
        raise HTTPException(status_code=503, detail="Model not loaded yet")

    if task:
        if task not in TASK_INSTRUCTIONS:
            raise HTTPException(status_code=400, detail=f"Unknown task '{task}'. Valid tasks: {list(TASK_INSTRUCTIONS.keys())}")
        resolved_instruction = TASK_INSTRUCTIONS[task]
        is_object_removal = False
    elif instruction:
        resolved_instruction = instruction
        is_object_removal = True
    else:
        raise HTTPException(status_code=400, detail="Provide either 'task' or 'instruction'.")

    data = await file.read()
    try:
        image = Image.open(io.BytesIO(data))
    except Exception as e:
        raise HTTPException(status_code=400, detail=f"Could not read image: {e}")

    fixed = await run_in_threadpool(
        runner.apply, image, resolved_instruction, "", is_object_removal
    )

    buffer = io.BytesIO()
    fixed.save(buffer, format="JPEG", quality=95)
    return Response(content=buffer.getvalue(), media_type="image/jpeg")

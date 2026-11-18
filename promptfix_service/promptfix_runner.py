"""
Load-once, serve-many wrapper around PromptFix (https://github.com/yeates/PromptFix).

PromptFix ships only as a directory-batch script (process_images_json.py) that loads
its diffusion model fresh on every invocation and processes a whole folder driven by
an instructions.json file. This module lifts the same model-loading and single-sample
denoising logic (copied from process_images_json.py's main()) into a class that loads
the checkpoint once and exposes apply(image, instruction) for repeated use from a
long-lived server, for any of PromptFix's supported edit instructions (not just deblur).

Requires a local clone of the PromptFix repo (with its stable_diffusion/ submodule and
checkpoints/promptfix.ckpt present) — see README.md for setup. PROMPTFIX_DIR must point
to that clone so this module can import stable_diffusion.ldm.* the same way the
upstream script does via sys.path.
"""

import os
import sys

import einops
import numpy as np
import torch
import torch.nn as nn
from einops import rearrange
from omegaconf import OmegaConf
from PIL import Image, ImageOps
from torch import autocast

# Canonical one-click instructions, taken from PromptFix's own
# examples/validation/instructions.json phrasing per task category. Object removal has
# no fixed instruction — it always needs a user-supplied target description.
TASK_INSTRUCTIONS = {
    "colorize": "Enrich this photo with a myriad of colors.",
    "dehaze": "Cleanse the photo of its hazy appearance.",
    "deblur": "Remove the blur from the image.",
    "dewatermark": "Remove watermark from the image.",
    "desnow": "Remove snow from the image.",
    "lowlight": "Enhance overall lighting of the image.",
}

DEFAULT_FLAW_PROMPT = ""

# Object removal ("inpaint" in PromptFix's own naming) uses more denoising steps and a
# different resize strategy than the other tasks, per process_images_json.py's
# per-filename special-casing (`'inpaint' not in image_path`).
OBJECT_REMOVAL_STEPS = 50


def resize_image_to_resolution(input_image, resolution, reverse=True):
    width, height = input_image.size
    scale = resolution / min(width, height) if reverse else resolution / max(width, height)
    new_width = (int(width * scale) // 64) * 64
    new_height = (int(height * scale) // 64) * 64
    return ImageOps.fit(input_image, (new_width, new_height), method=Image.Resampling.LANCZOS)


class CFGDenoiser(nn.Module):
    """Verbatim from PromptFix's process_images_json.py, minus the optional
    high-frequency-preservation guidance path (disabled by default upstream too)."""

    def __init__(self, model):
        super().__init__()
        self.inner_model = model

    def forward(self, z, sigma, cond, uncond, text_cfg_scale, image_cfg_scale, input_image=None):
        cfg_z = einops.repeat(z, "b ... -> (repeat b) ...", repeat=3)
        cfg_sigma = einops.repeat(sigma, "b ... -> (repeat b) ...", repeat=3)
        cfg_cond = {
            "c_crossattn": [torch.cat([cond["c_crossattn"][0], uncond["c_crossattn"][0], cond["c_crossattn"][0]])],
            "c_concat": [torch.cat([cond["c_concat"][0], cond["c_concat"][0], uncond["c_concat"][0]])],
        }
        out_cond, out_img_cond, out_txt_cond = self.inner_model(cfg_z, cfg_sigma, cond=cfg_cond).chunk(3)
        return 0.5 * (out_img_cond + out_txt_cond) + text_cfg_scale * (out_cond - out_img_cond) + image_cfg_scale * (
            out_cond - out_txt_cond
        )


class PromptFixRunner:
    def __init__(
        self,
        promptfix_dir,
        config_path="configs/promptfix.yaml",
        ckpt_path="checkpoints/promptfix.ckpt",
        resolution=640,
        steps=20,
        cfg_text=6.5,
        cfg_image=1.25,
        seed=2024,
    ):
        # promptfix_dir itself must be on sys.path for `stable_diffusion.ldm.*` to resolve
        # (stable_diffusion/ has no __init__.py, so it's an implicit namespace package);
        # promptfix_dir/stable_diffusion must also be on sys.path because modules inside
        # it use bare `from ldm.xxx import ...` internally (matching how the upstream
        # process_images_json.py script relies on both being importable from repo root).
        sys.path.append(promptfix_dir)
        sys.path.append(os.path.join(promptfix_dir, "stable_diffusion"))

        # taming-transformers' `pip install -e git+...` editable install can produce an
        # empty package mapping on some setuptools versions (its setup.py doesn't declare
        # `packages=` explicitly), leaving `taming` unimportable despite being "installed".
        # Fall back to pip's default editable-checkout location for git installs.
        try:
            import taming  # noqa: F401
        except ImportError:
            taming_src = os.path.join(sys.prefix, "src", "taming-transformers")
            if os.path.isdir(os.path.join(taming_src, "taming")):
                sys.path.append(taming_src)

        # Deferred import: only resolvable once the paths above are on sys.path.
        import k_diffusion as K
        from stable_diffusion.ldm.util import instantiate_from_config

        self._K = K
        self.resolution = resolution
        self.steps = steps
        self.cfg_text = cfg_text
        self.cfg_image = cfg_image
        self.seed = seed

        config = OmegaConf.load(os.path.join(promptfix_dir, config_path))
        model = instantiate_from_config(config.model)
        state_dict = torch.load(os.path.join(promptfix_dir, ckpt_path), map_location="cpu")
        if "state_dict" in state_dict:
            state_dict = state_dict["state_dict"]
        model.load_state_dict(state_dict, strict=False)

        self.device = "cuda" if torch.cuda.is_available() else "cpu"
        self.model = model.eval().to(self.device)
        self.model_wrap = K.external.CompVisDenoiser(self.model)
        self.model_wrap_cfg = CFGDenoiser(self.model_wrap)
        self.null_token = self.model.get_learned_conditioning([""])

    def apply(
        self,
        image: Image.Image,
        instruction: str,
        flaw_prompt: str = DEFAULT_FLAW_PROMPT,
        is_object_removal: bool = False,
    ) -> Image.Image:
        """Run a single-sample PromptFix denoise pass for an arbitrary edit instruction.

        `instruction` can be any free-text edit instruction PromptFix understands, not
        just the canonical one-click tasks in TASK_INSTRUCTIONS. `is_object_removal`
        mirrors process_images_json.py's per-filename special-casing for its "inpaint"
        examples: more denoising steps, and a resize strategy that fits the image's
        longer side to the target resolution instead of the shorter side.
        """
        K = self._K
        steps = OBJECT_REMOVAL_STEPS if is_object_removal else self.steps
        input_image = resize_image_to_resolution(
            image.convert("RGB"), self.resolution, reverse=not is_object_removal
        )

        with autocast(self.device):
            cond = {}
            cond["c_crossattn"] = [
                self.model.get_learned_conditioning([instruction, flaw_prompt])
            ]
            image_tensor = 2 * torch.tensor(np.array(input_image)).float() / 255 - 1
            image_tensor = rearrange(image_tensor, "h w c -> 1 c h w").to(self.device)
            cond["c_concat"] = [self.model.encode_first_stage(image_tensor).mode()]

            uncond = {
                "c_crossattn": [torch.cat([self.null_token, self.null_token], 0)],
                "c_concat": [torch.zeros_like(cond["c_concat"][0])],
            }

            sigmas = self.model_wrap.get_sigmas(steps)
            extra_args = {
                "cond": cond,
                "uncond": uncond,
                "text_cfg_scale": self.cfg_text,
                "image_cfg_scale": self.cfg_image,
            }

            torch.manual_seed(self.seed)
            _, skip_connect_hs = self.model.first_stage_model.encoder(image_tensor)
            z = torch.randn_like(cond["c_concat"][0]) * sigmas[0]
            z = K.sampling.sample_euler_ancestral(self.model_wrap_cfg, z, sigmas, extra_args=extra_args)
            x = self.model.decode_first_stage(z, skip_connect_hs=skip_connect_hs)
            x = torch.clamp((x + 1.0) / 2.0, min=0.0, max=1.0)
            x = 255.0 * rearrange(x, "1 c h w -> h w c")
            return Image.fromarray(x.type(torch.uint8).cpu().numpy())

#!/usr/bin/env python3
"""
Local image-generation server using MFLUX (MLX-native FLUX implementation).

MFLUX is actively maintained and works with current MLX versions, unlike the
now-archived DiffusionKit package. This server exposes an OpenAI-style
POST /v1/images/generations endpoint.
"""

import argparse
import base64
import io
import json
import os
import sys
import time
import traceback
import urllib.parse
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

_PROJECT_ROOT = Path(__file__).resolve().parents[3]

# Public, pre-quantized 4-bit community model. No HuggingFace gating required.
DEFAULT_MODEL_PATH = os.environ.get(
    "SOTF_MFLUX_MODEL_PATH", "dhairyashil/FLUX.1-schnell-mflux-v0.6.2-4bit"
)
DEFAULT_BASE_MODEL = os.environ.get("SOTF_MFLUX_BASE_MODEL", "schnell")
DEFAULT_CACHE_DIR = os.environ.get(
    "SOTF_MODEL_CACHE_DIR", str(_PROJECT_ROOT / "data_cached" / "models")
)


def parse_args():
    parser = argparse.ArgumentParser(
        description="Local MLX image-generation server for cli.py (MFLUX backend)"
    )
    parser.add_argument("--host", default="127.0.0.1", help="Bind host")
    parser.add_argument("--port", type=int, default=0, help="Bind port (0 = auto)")
    parser.add_argument(
        "--model-path",
        default=DEFAULT_MODEL_PATH,
        help="Hugging Face repo id or local path to the MFLUX model weights",
    )
    parser.add_argument(
        "--base-model",
        default=DEFAULT_BASE_MODEL,
        choices=["schnell", "dev", "dev-kontext", "krea-dev", "flux2-klein-4b", "flux2-klein-9b"],
        help="MFLUX base model configuration",
    )
    parser.add_argument(
        "--quantize",
        type=int,
        default=None,
        help="Runtime quantization bits (3/4/6/8). Omit for pre-quantized models.",
    )
    parser.add_argument(
        "--low-ram",
        action="store_true",
        help="Reduce peak memory by releasing components after use",
    )
    parser.add_argument(
        "--cache-dir",
        default=DEFAULT_CACHE_DIR,
        help="Directory for downloaded model weights",
    )
    parser.add_argument(
        "--steps", type=int, default=4, help="Number of denoising steps"
    )
    parser.add_argument(
        "--guidance",
        type=float,
        default=3.5,
        help="Guidance scale (ignored for schnell)",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Random seed (default: random)",
    )
    return parser.parse_args()


def load_pipeline(args):
    try:
        from mflux.models.flux.variants.txt2img.flux import Flux1
        from mflux.models.common.config.model_config import ModelConfig
    except ImportError as exc:
        print(f"[mflux_server] missing dependency: {exc}", file=sys.stderr)
        raise SystemExit(1)

    cache_dir = Path(args.cache_dir).expanduser().resolve()
    cache_dir.mkdir(parents=True, exist_ok=True)
    os.environ.setdefault("HF_HOME", str(cache_dir))

    print(f"[mflux_server] loading {args.model_path} (base {args.base_model})", file=sys.stderr)
    print(f"[mflux_server] cache dir: {cache_dir}", file=sys.stderr)

    model_config = ModelConfig.from_name(
        model_name=args.base_model,
        base_model=None,
    )
    flux = Flux1(
        model_config=model_config,
        model_path=args.model_path,
        quantize=args.quantize,
    )
    print("[mflux_server] ready", file=sys.stderr)
    return flux


def parse_size(size: str):
    try:
        width_text, height_text = size.lower().split("x", 1)
        return int(width_text), int(height_text)
    except Exception:
        return 1024, 1024


def make_handler(flux, args):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            pass

        def do_POST(self):
            if urllib.parse.urlparse(self.path).path != "/v1/images/generations":
                self.send_error(404, message="not found")
                return

            try:
                length = int(self.headers.get("Content-Length", "0"))
                body = self.rfile.read(length).decode("utf-8")
                payload = json.loads(body) if body else {}
                prompt = payload.get("prompt", "")

                width, height = parse_size(payload.get("size", "1024x1024"))
                # MFLUX requires dimensions divisible by 16.
                width = (width // 16) * 16
                height = (height // 16) * 16
                if width < 256:
                    width = 256
                if height < 256:
                    height = 256

                seed = payload.get("seed", args.seed)
                if seed is None:
                    seed = int(time.time()) % (2**31)

                print(
                    f"[mflux_server] generating {width}x{height} for: {prompt[:80]!r}",
                    file=sys.stderr,
                )

                result = flux.generate_image(
                    seed=seed,
                    prompt=prompt,
                    num_inference_steps=args.steps,
                    height=height,
                    width=width,
                    guidance=args.guidance,
                )

                buffer = io.BytesIO()
                result.image.save(buffer, format="PNG")
                b64 = base64.b64encode(buffer.getvalue()).decode("ascii")

                response = {
                    "created": int(time.time()),
                    "data": [{"b64_json": b64}],
                }
                data = json.dumps(response).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
            except Exception as exc:
                tb = traceback.format_exc()
                print(
                    f"[mflux_server] generation failed: {exc}\n{tb}",
                    file=sys.stderr,
                )
                self.send_error(500, message=f"{exc}")

    return Handler


def main():
    args = parse_args()
    flux = load_pipeline(args)
    server = HTTPServer((args.host, args.port), make_handler(flux, args))
    host, port = server.server_address
    print(f"[mflux_server] listening on http://{host}:{port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()

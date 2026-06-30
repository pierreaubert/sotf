#!/usr/bin/env python3
"""
Minimal local HTTP server for FLUX image generation using PyTorch + Diffusers.

Exposes an OpenAI-compatible POST /v1/images/generations endpoint and returns
images as base64-encoded PNG data. Intended to be spawned by cli.py when no
external image server is reachable.
"""

import argparse
import base64
import io
import json
import os
import sys
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

_PROJECT_ROOT = Path(__file__).resolve().parents[3]

DEFAULT_CACHE_DIR = os.environ.get(
    "SOTF_MODEL_CACHE_DIR", str(_PROJECT_ROOT / "data_cached" / "models")
)


def parse_args():
    parser = argparse.ArgumentParser(
        description="Local FLUX image generation server for cli.py"
    )
    parser.add_argument("--host", default="127.0.0.1", help="Bind host")
    parser.add_argument("--port", type=int, default=0, help="Bind port (0 = auto)")
    parser.add_argument(
        "--model",
        default="black-forest-labs/FLUX.1-schnell",
        help="Hugging Face model id or local path for the diffusers pipeline",
    )
    parser.add_argument(
        "--dtype",
        default="bfloat16",
        choices=["bfloat16", "float16", "float32"],
        help="Torch dtype for the pipeline",
    )
    parser.add_argument(
        "--device",
        default=None,
        help="Torch device (cuda, mps, cpu). Auto-detected if omitted.",
    )
    parser.add_argument(
        "--steps", type=int, default=4, help="Number of inference steps"
    )
    parser.add_argument(
        "--guidance-scale",
        type=float,
        default=0.0,
        help="Guidance scale (FLUX schnell typically uses 0.0)",
    )
    parser.add_argument(
        "--cache-dir",
        default=DEFAULT_CACHE_DIR,
        help="Directory for downloaded model weights (sets HF_HUB_CACHE)",
    )
    return parser.parse_args()


def load_pipeline(args):
    try:
        import torch
        from diffusers import FluxPipeline
    except ImportError as exc:
        print(f"[flux2_server] missing dependency: {exc}", file=sys.stderr)
        raise SystemExit(1)

    cache_dir = Path(args.cache_dir).expanduser().resolve()
    cache_dir.mkdir(parents=True, exist_ok=True)
    os.environ["HF_HUB_CACHE"] = str(cache_dir)

    dtype_map = {
        "bfloat16": torch.bfloat16,
        "float16": torch.float16,
        "float32": torch.float32,
    }
    dtype = dtype_map[args.dtype]

    device = args.device
    if device is None:
        if torch.cuda.is_available():
            device = "cuda"
        elif torch.backends.mps.is_available():
            device = "mps"
        else:
            device = "cpu"

    print(
        f"[flux2_server] loading {args.model} on {device} with {args.dtype}",
        file=sys.stderr,
    )
    print(f"[flux2_server] cache dir: {cache_dir}", file=sys.stderr)
    pipe = FluxPipeline.from_pretrained(
        args.model, torch_dtype=dtype, cache_dir=str(cache_dir)
    )
    pipe = pipe.to(device)
    print("[flux2_server] ready", file=sys.stderr)
    return pipe


def parse_size(size: str):
    try:
        width_text, height_text = size.lower().split("x", 1)
        return int(width_text), int(height_text)
    except Exception:
        return 1024, 1024


def make_handler(pipe, args):
    import torch

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

                print(
                    f"[flux2_server] generating {width}x{height} for: {prompt[:80]!r}",
                    file=sys.stderr,
                )
                with torch.no_grad():
                    image = pipe(
                        prompt,
                        height=height,
                        width=width,
                        num_inference_steps=args.steps,
                        guidance_scale=args.guidance_scale,
                        max_sequence_length=256,
                    ).images[0]

                buffer = io.BytesIO()
                image.save(buffer, format="PNG")
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
                print(f"[flux2_server] generation failed: {exc}", file=sys.stderr)
                self.send_error(500, message=str(exc))

    return Handler


def main():
    args = parse_args()
    pipe = load_pipeline(args)
    server = HTTPServer((args.host, args.port), make_handler(pipe, args))
    host, port = server.server_address
    print(f"[flux2_server] listening on http://{host}:{port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()

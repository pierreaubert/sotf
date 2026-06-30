#!/usr/bin/env python3
"""
Local Gemma text-generation server for cli.py.

Loads a Gemma causal-LM via Transformers or mlx-lm and exposes a POST
/api/v1/chat endpoint compatible with the LM Studio format used by cli.py:

  {"model": "...", "system_prompt": "...", "input": "..."}

Response shape mirrors OpenAI chat completions so cli.py's response parser
continues to work unchanged.
"""

import argparse
import json
import os
import sys
import urllib.parse
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

_PROJECT_ROOT = Path(__file__).resolve().parents[3]

DEFAULT_MODEL = os.environ.get(
    "SOTF_GEMMA_MODEL", "mlx-community/gemma-4-12b-4bit"
)
DEFAULT_CACHE_DIR = os.environ.get(
    "SOTF_MODEL_CACHE_DIR", str(_PROJECT_ROOT / "data_cached" / "models")
)


def parse_args():
    parser = argparse.ArgumentParser(
        description="Local Gemma text-generation server for cli.py"
    )
    parser.add_argument("--host", default="127.0.0.1", help="Bind host")
    parser.add_argument("--port", type=int, default=0, help="Bind port (0 = auto)")
    parser.add_argument(
        "--model",
        default=DEFAULT_MODEL,
        help="Hugging Face model id or local path for the Gemma model",
    )
    parser.add_argument(
        "--cache-dir",
        default=DEFAULT_CACHE_DIR,
        help="Directory for downloaded model weights (sets HF_HUB_CACHE)",
    )
    parser.add_argument(
        "--backend",
        default="auto",
        choices=["auto", "transformers", "mlx"],
        help=(
            "Inference backend. 'auto' prefers mlx on Apple Silicon and "
            "transformers otherwise."
        ),
    )
    parser.add_argument(
        "--dtype",
        default="bfloat16",
        choices=["bfloat16", "float16", "float32"],
        help="Torch dtype for the transformers backend",
    )
    parser.add_argument(
        "--device",
        default=None,
        help="Torch device for the transformers backend (cuda/mps/cpu). Auto-detected if empty.",
    )
    parser.add_argument(
        "--max-new-tokens",
        type=int,
        default=512,
        help="Maximum number of new tokens to generate",
    )
    return parser.parse_args()


def resolve_backend(args):
    if args.backend != "auto":
        return args.backend

    model_lower = args.model.lower()
    looks_like_mlx_model = "mlx" in model_lower or "-4bit" in model_lower

    if sys.platform == "darwin" or looks_like_mlx_model:
        try:
            import mlx_lm  # noqa: F401
            return "mlx"
        except Exception:
            if looks_like_mlx_model:
                print(
                    "[gemma_server] this model appears to be an MLX-quantized model; "
                    "install mlx-lm to load it: pip install mlx-lm",
                    file=sys.stderr,
                )
                raise SystemExit(1)
    return "transformers"


def setup_cache(args):
    cache_dir = Path(args.cache_dir).expanduser().resolve()
    cache_dir.mkdir(parents=True, exist_ok=True)
    os.environ["HF_HUB_CACHE"] = str(cache_dir)
    return cache_dir


def load_model_transformers(args):
    try:
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer
    except ImportError as exc:
        print(f"[gemma_server] missing dependency: {exc}", file=sys.stderr)
        raise SystemExit(1)

    cache_dir = setup_cache(args)
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
        f"[gemma_server] loading {args.model} on {device} with {args.dtype} (transformers)",
        file=sys.stderr,
    )
    print(f"[gemma_server] cache dir: {cache_dir}", file=sys.stderr)

    tokenizer = AutoTokenizer.from_pretrained(args.model, cache_dir=str(cache_dir))
    model_kwargs = {
        "torch_dtype": dtype,
        "cache_dir": str(cache_dir),
    }
    if device == "cuda":
        model_kwargs["device_map"] = "auto"
    model = AutoModelForCausalLM.from_pretrained(args.model, **model_kwargs)
    if device != "cuda":
        model = model.to(device)

    print("[gemma_server] ready", file=sys.stderr)
    return model, tokenizer, device


def load_model_mlx(args):
    try:
        from mlx_lm import load
    except ImportError as exc:
        print(f"[gemma_server] missing dependency: {exc}", file=sys.stderr)
        print(
            "[gemma_server] install mlx-lm to use the mlx backend on Apple Silicon",
            file=sys.stderr,
        )
        raise SystemExit(1)

    cache_dir = setup_cache(args)
    print(
        f"[gemma_server] loading {args.model} (mlx)",
        file=sys.stderr,
    )
    print(f"[gemma_server] cache dir: {cache_dir}", file=sys.stderr)

    model, tokenizer = load(args.model)
    print("[gemma_server] ready", file=sys.stderr)
    return model, tokenizer


def build_prompt(tokenizer, system_prompt: str, user_input: str) -> str:
    messages = []
    if system_prompt:
        messages.append({"role": "system", "content": system_prompt})
    messages.append({"role": "user", "content": user_input})

    chat_template = getattr(tokenizer, "chat_template", None)
    if chat_template is not None:
        try:
            return tokenizer.apply_chat_template(
                messages, tokenize=False, add_generation_prompt=True
            )
        except Exception:
            pass

    if system_prompt:
        return f"{system_prompt}\n\n{user_input}".strip()
    return user_input.strip()


def make_handler_transformers(model, tokenizer, device, args):
    import torch

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            pass

        def do_POST(self):
            if urllib.parse.urlparse(self.path).path != "/api/v1/chat":
                self.send_error(404, message="not found")
                return

            try:
                length = int(self.headers.get("Content-Length", "0"))
                body = self.rfile.read(length).decode("utf-8")
                payload = json.loads(body) if body else {}
                system_prompt = payload.get("system_prompt", "")
                user_input = payload.get("input", "")

                prompt = build_prompt(tokenizer, system_prompt, user_input)
                print(
                    f"[gemma_server] generating for: {prompt[:80]!r}",
                    file=sys.stderr,
                )

                inputs = tokenizer(prompt, return_tensors="pt").to(device)
                with torch.no_grad():
                    outputs = model.generate(
                        **inputs,
                        max_new_tokens=args.max_new_tokens,
                        do_sample=True,
                        temperature=0.7,
                        top_p=0.9,
                    )
                generated_tokens = outputs[0][inputs["input_ids"].shape[1] :]
                text = tokenizer.decode(generated_tokens, skip_special_tokens=True)
                text = text.strip()

                response = {"choices": [{"message": {"content": text}}]}
                data = json.dumps(response).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
            except Exception as exc:
                print(f"[gemma_server] generation failed: {exc}", file=sys.stderr)
                self.send_error(500, message=str(exc))

    return Handler


def make_handler_mlx(model, tokenizer, args):
    from mlx_lm import generate

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            pass

        def do_POST(self):
            if urllib.parse.urlparse(self.path).path != "/api/v1/chat":
                self.send_error(404, message="not found")
                return

            try:
                length = int(self.headers.get("Content-Length", "0"))
                body = self.rfile.read(length).decode("utf-8")
                payload = json.loads(body) if body else {}
                system_prompt = payload.get("system_prompt", "")
                user_input = payload.get("input", "")

                prompt = build_prompt(tokenizer, system_prompt, user_input)
                print(
                    f"[gemma_server] generating for: {prompt[:80]!r}",
                    file=sys.stderr,
                )

                text = generate(
                    model,
                    tokenizer,
                    prompt=prompt,
                    max_tokens=args.max_new_tokens,
                    temp=0.7,
                    top_p=0.9,
                )
                text = text.strip()

                response = {"choices": [{"message": {"content": text}}]}
                data = json.dumps(response).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
            except Exception as exc:
                print(f"[gemma_server] generation failed: {exc}", file=sys.stderr)
                self.send_error(500, message=str(exc))

    return Handler


def main():
    args = parse_args()
    backend = resolve_backend(args)
    if backend == "mlx":
        model, tokenizer = load_model_mlx(args)
        handler = make_handler_mlx(model, tokenizer, args)
    else:
        model, tokenizer, device = load_model_transformers(args)
        handler = make_handler_transformers(model, tokenizer, device, args)

    server = HTTPServer((args.host, args.port), handler)
    host, port = server.server_address
    print(f"[gemma_server] listening on http://{host}:{port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()

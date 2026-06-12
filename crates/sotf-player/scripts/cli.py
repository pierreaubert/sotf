#!/usr/bin/env python3
"""
Generate missing album art.

Modes:
  python3 cli.py -p "direct prompt"
  python3 cli.py /path/to/album-or-music-tree --limit 10
"""

import argparse
import base64
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path

try:
    from mutagen import File as MutagenFile
except Exception:
    MutagenFile = None


AUDIO_EXTENSIONS = {
    ".aac",
    ".aif",
    ".aiff",
    ".alac",
    ".flac",
    ".m4a",
    ".mp3",
    ".ogg",
    ".opus",
    ".wav",
    ".wma",
}
IMAGE_EXTENSIONS = {".avif", ".bmp", ".gif", ".jpeg", ".jpg", ".png", ".webp"}
COVER_BASENAMES = {
    "album",
    "albumart",
    "cover",
    "folder",
    "front",
}
ART_SUBDIRS = {"art", "artwork", "cover", "covers", "scan", "scans"}
DEFAULT_OUTPUT_NAME = "cover.png"
DEFAULT_IMAGE_URL = "http://192.168.1.37:9999/v1/images/generations"
DEFAULT_COMPLETION_URL = "http://localhost:1234/api/v1/chat"
DEFAULT_TEXT_MODEL = "google/gemma-4-12b-qat"
DEFAULT_IMAGE_MODEL = "zai-org/GLM-Image"
DEFAULT_SYSTEM_PROMPT_PATH = Path("crates/sotf-player/prompts/system-prompt.md")


@dataclass
class AlbumCandidate:
    directory: Path
    output_path: Path
    short_content: str


def parse_args():
    parser = argparse.ArgumentParser(
        description="Generate missing album art from album folders",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python3 cli.py -p "minimal blue square cover" --save /tmp/cover.png
  python3 cli.py /Volumes/Music/Album --dry-run
  python3 cli.py /Volumes/Music --limit 10 --trace-http
        """,
    )
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="Album directory or recursive music directory to scan",
    )
    parser.add_argument(
        "-p",
        "--prompt",
        type=str,
        default=None,
        help="Direct image prompt. If paths are also provided, this becomes a prompt override.",
    )
    parser.add_argument(
        "--expand-direct-prompt",
        action="store_true",
        help="Run --prompt through the text model before image generation.",
    )

    parser.add_argument(
        "--completion-url",
        default=os.environ.get("COMPLETION_URL", DEFAULT_COMPLETION_URL),
        help="Text completion endpoint URL",
    )
    parser.add_argument(
        "--completion-model",
        default=os.environ.get("COMPLETION_MODEL", DEFAULT_TEXT_MODEL),
        help="Text model used to expand album metadata into an image prompt",
    )
    parser.add_argument(
        "--completion-api-key-env",
        default="OPENAI_API_KEY",
        help="Environment variable for text model auth. Empty means no Authorization header.",
    )
    parser.add_argument(
        "--system-prompt-file",
        type=Path,
        default=DEFAULT_SYSTEM_PROMPT_PATH,
        help="System prompt file for text prompt expansion",
    )

    parser.add_argument(
        "-m",
        "--model",
        default=os.environ.get("GLM_IMAGE_MODEL", DEFAULT_IMAGE_MODEL),
        choices=[
            "glm-image",
            "zai-org/GLM-Image",
            "cogview-4-250304",
            "cogview-4",
            "cogview-3-flash",
        ],
        help="Image model to use",
    )
    parser.add_argument(
        "--url",
        default=os.environ.get("GLM_IMAGE_URL", DEFAULT_IMAGE_URL),
        help="Image generation endpoint URL",
    )
    parser.add_argument(
        "--api-key-env",
        default="ZHIPU_API_KEY",
        help="Environment variable for image model auth. Empty means no Authorization header.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=float(os.environ.get("GLM_IMAGE_TIMEOUT", "600")),
        help="HTTP timeout in seconds",
    )
    parser.add_argument("-s", "--size", default="1280x1280", help="Image size")
    parser.add_argument(
        "-q",
        "--quality",
        default="hd",
        choices=["hd", "standard"],
        help="Image quality",
    )
    parser.add_argument("--no-watermark", action="store_true", help="Disable watermark")
    parser.add_argument("--user-id", default=None, help="End-user ID, 6 to 128 chars")
    parser.add_argument("--save", default=None, help="Save direct-prompt image to this file")

    parser.add_argument("--output-name", default=DEFAULT_OUTPUT_NAME)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--trace-http", action="store_true")
    parser.add_argument("--verbose", "-v", action="store_true")

    return parser.parse_args()


def eprint(*parts):
    print(*parts, file=sys.stderr)


def json_exit(payload, code=0):
    print(json.dumps(payload, ensure_ascii=False))
    raise SystemExit(code)


def read_system_prompt(path: Path) -> str:
    if path.exists():
        return path.read_text(encoding="utf-8").strip()
    return (
        "You are a senior prompt designer specializing in album cover art. "
        "Return only one concise text-to-image prompt, with no commentary."
    )


def validate_size(size: str, model: str = "glm-image") -> bool:
    try:
        width_text, height_text = size.split("x", 1)
        width = int(width_text)
        height = int(height_text)
    except (ValueError, IndexError):
        return False

    if model in {"glm-image", "zai-org/GLM-Image"}:
        return (
            width % 32 == 0
            and height % 32 == 0
            and 1024 <= width <= 2048
            and 1024 <= height <= 2048
            and width * height <= 2**22
        )

    return (
        width % 16 == 0
        and height % 16 == 0
        and 512 <= width <= 2048
        and 512 <= height <= 2048
        and width * height <= 2**21
    )


def validate_user_id(user_id: str) -> bool:
    return not user_id or 6 <= len(user_id) <= 128


def http_json(url, payload, api_key="", timeout=120, trace=False, label="request"):
    data = json.dumps(payload).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"

    if trace:
        eprint(f"[cli.py] {label}: POST {url}")
        eprint(json.dumps(payload, indent=2, ensure_ascii=False))

    req = urllib.request.Request(url, data=data, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as response:
            body = response.read().decode("utf-8")
            if trace:
                eprint(f"[cli.py] {label}: HTTP {response.status}, {len(body)} bytes")
                eprint(body[:4000])
            return {"ok": True, "body": json.loads(body)}
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8") if error.fp else ""
        if trace:
            eprint(f"[cli.py] {label}: HTTP {error.code}")
            eprint(body[:4000])
        return {
            "ok": False,
            "error": {"code": str(error.code), "message": body or str(error), "status": error.code},
        }
    except urllib.error.URLError as error:
        return {"ok": False, "error": {"code": "NETWORK_ERROR", "message": str(error.reason)}}
    except Exception as error:
        return {"ok": False, "error": {"code": "UNKNOWN_ERROR", "message": str(error)}}


def extract_text_response(response):
    if isinstance(response, str):
        return response.strip()
    if not isinstance(response, dict):
        return ""

    output_text = response.get("output_text")
    if isinstance(output_text, str) and output_text.strip():
        return output_text.strip()

    output = response.get("output")
    if isinstance(output, list):
        extracted = extract_text_from_output_list(output)
        if extracted:
            return extracted

    for key in ("response", "text", "content", "message"):
        value = response.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
        extracted = extract_text_from_parts(value)
        if extracted:
            return extracted

    message = response.get("message")
    if isinstance(message, dict) and isinstance(message.get("content"), str):
        return message["content"].strip()
    extracted = extract_text_from_parts(message)
    if extracted:
        return extracted

    choices = response.get("choices")
    if isinstance(choices, list) and choices:
        message = choices[0].get("message") if isinstance(choices[0], dict) else None
        if isinstance(message, dict) and isinstance(message.get("content"), str):
            return message["content"].strip()
        extracted = extract_text_from_parts(message)
        if extracted:
            return extracted

    return ""


def extract_text_from_output_list(items):
    non_reasoning = []
    reasoning = []
    for item in items:
        if not isinstance(item, dict):
            text = extract_text_from_parts(item)
            if text:
                non_reasoning.append(text)
            continue
        text = extract_text_from_parts(item)
        if not text:
            continue
        if item.get("type") == "reasoning":
            reasoning.append(text)
        else:
            non_reasoning.append(text)
    if non_reasoning:
        return "\n".join(non_reasoning).strip()
    return "\n".join(reasoning).strip()


def extract_text_from_parts(value):
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, dict):
        for key in ("text", "content", "output_text"):
            text = value.get(key)
            if isinstance(text, str) and text.strip():
                return text.strip()
            extracted = extract_text_from_parts(text)
            if extracted:
                return extracted
        return ""
    if isinstance(value, list):
        fragments = []
        for item in value:
            extracted = extract_text_from_parts(item)
            if extracted:
                fragments.append(extracted)
        return "\n".join(fragments).strip()
    return ""


def strip_code_fence(text):
    text = text.strip()
    if not text.startswith("```"):
        return text
    inner = text[3:]
    end = inner.rfind("```")
    if end == -1:
        return text
    fenced = inner[:end].strip()
    lines = fenced.splitlines()
    if lines and re.fullmatch(r"[A-Za-z]+", lines[0].strip()):
        return "\n".join(lines[1:]).strip()
    return fenced


def normalize_prompt(text):
    text = strip_code_fence(text).strip()
    if text.startswith("{"):
        try:
            parsed = json.loads(text)
            if isinstance(parsed, dict) and isinstance(parsed.get("prompt"), str):
                return parsed["prompt"].strip()
        except json.JSONDecodeError:
            pass
    return final_prompt_paragraph(text)


def final_prompt_paragraph(text):
    paragraphs = [part.strip() for part in re.split(r"\n\s*\n", text) if part.strip()]
    for paragraph in reversed(paragraphs):
        cleaned = paragraph.strip().strip('"')
        if not cleaned:
            continue
        if cleaned.startswith(("*", "-", "Actually,", "Wait,", "Check ", "One detail:", "Let's go.")):
            continue
        if len(cleaned) >= 40:
            return cleaned
    return text.strip()


def expand_prompt(args, short_content):
    system_prompt = read_system_prompt(args.system_prompt_file)
    user_prompt = (
        "Create one text-to-image prompt from this album metadata.\n\n"
        f"{short_content}\n\n"
        "The image must be square album art, visually specific, tasteful, and usable as "
        "cover artwork. Include a compact negative instruction at the end."
    )
    payload = {
        "model": args.completion_model,
        "system_prompt": system_prompt,
        "input": user_prompt,
    }
    api_key = os.environ.get(args.completion_api_key_env, "").strip()
    response = http_json(
        args.completion_url,
        payload,
        api_key=api_key,
        timeout=args.timeout,
        trace=args.trace_http,
        label="completion",
    )
    if not response["ok"]:
        return response
    prompt = normalize_prompt(extract_text_response(response["body"]))
    if not prompt:
        return {"ok": False, "error": {"code": "EMPTY_PROMPT", "message": "text model returned no text"}}
    return {"ok": True, "prompt": prompt}


def generate_image(args, prompt: str):
    payload = {
        "model": args.model,
        "prompt": prompt,
        "size": args.size,
        "quality": args.quality,
        "watermark_enabled": not args.no_watermark,
    }
    if args.user_id:
        payload["user_id"] = args.user_id

    api_key = os.environ.get(args.api_key_env, "").strip()
    response = http_json(
        args.url,
        payload,
        api_key=api_key,
        timeout=args.timeout,
        trace=args.trace_http,
        label="image",
    )
    if not response["ok"]:
        return response

    body = response["body"]
    data = body.get("data") if isinstance(body, dict) else None
    image = data[0] if isinstance(data, list) and data else {}
    image_url = image.get("url") if isinstance(image, dict) else None
    image_b64 = image.get("b64_json") if isinstance(image, dict) else None

    return {
        "ok": True,
        "raw": body,
        "image_url": urllib.parse.urljoin(args.url, image_url) if image_url else None,
        "image_b64": image_b64,
        "created": body.get("created") if isinstance(body, dict) else None,
        "content_filter": body.get("content_filter", []) if isinstance(body, dict) else [],
        "prompt": prompt,
        "size": args.size,
        "quality": args.quality,
    }


def write_image_result(result, path: Path):
    if result.get("image_b64"):
        path.write_bytes(base64.b64decode(result["image_b64"]))
        return True
    if result.get("image_url"):
        with urllib.request.urlopen(result["image_url"], timeout=120) as response:
            path.write_bytes(response.read())
        return True
    return False


def clean_label(value):
    if value is None:
        return None
    if isinstance(value, (list, tuple)):
        value = value[0] if value else None
    if value is None:
        return None
    text = " ".join(str(value).split()).strip()
    return text[:160] if text else None


def audio_files_in(directory: Path):
    return sorted(
        path
        for path in directory.iterdir()
        if path.is_file() and path.suffix.lower() in AUDIO_EXTENSIONS
    )


def image_files_in(directory: Path):
    return sorted(
        path
        for path in directory.iterdir()
        if path.is_file() and path.suffix.lower() in IMAGE_EXTENSIONS
    )


def find_album_art(directory: Path):
    images = image_files_in(directory)
    for image in images:
        if image.stem.lower().replace(" ", "").replace("_", "").replace("-", "") in COVER_BASENAMES:
            return image
    if len(images) == 1:
        return images[0]

    for child in directory.iterdir():
        if child.is_dir() and child.name.lower() in ART_SUBDIRS:
            nested = image_files_in(child)
            if nested:
                return nested[0]
    return None


def read_tags(path: Path):
    if MutagenFile is None:
        return {}
    try:
        audio = MutagenFile(path, easy=True)
    except Exception:
        return {}
    if not audio or not audio.tags:
        return {}
    return {key.lower(): value for key, value in audio.tags.items()}


def first_tag(tags_by_file, *names):
    for tags in tags_by_file:
        for name in names:
            value = clean_label(tags.get(name))
            if value:
                return value
    return None


def unique_tags(tags_by_file, *names, limit=4):
    values = []
    seen = set()
    for tags in tags_by_file:
        for name in names:
            raw = tags.get(name)
            if raw is None:
                continue
            items = raw if isinstance(raw, (list, tuple)) else [raw]
            for item in items:
                value = clean_label(item)
                if value and value.lower() not in seen:
                    seen.add(value.lower())
                    values.append(value)
                    if len(values) >= limit:
                        return values
    return values


def title_from_filename(path: Path):
    title = re.sub(r"^\s*\d+[\s._-]+", "", path.stem)
    return clean_label(title) or path.stem


def build_short_content(directory: Path, tracks):
    tags_by_file = [read_tags(track) for track in tracks]
    album = first_tag(tags_by_file, "album") or clean_label(directory.name) or "Unknown Album"
    artist = (
        first_tag(tags_by_file, "albumartist", "album artist", "artist")
        or clean_label(directory.parent.name)
        or "Unknown Artist"
    )
    year = first_tag(tags_by_file, "date", "year")
    genres = unique_tags(tags_by_file, "genre", limit=4)
    composers = unique_tags(tags_by_file, "composer", limit=4)
    performers = unique_tags(tags_by_file, "performer", "artist", limit=4)
    track_titles = []
    seen = set()
    for track, tags in zip(tracks, tags_by_file):
        title = clean_label(tags.get("title")) or title_from_filename(track)
        key = title.lower()
        if key not in seen:
            seen.add(key)
            track_titles.append(title)
        if len(track_titles) >= 8:
            break

    lines = [f"Album: {album}", f"Artist: {artist}"]
    if year:
        lines.append(f"Year: {year}")
    if genres:
        lines.append(f"Genre: {', '.join(genres)}")
    if composers:
        lines.append(f"Composer: {', '.join(composers)}")
    if performers:
        lines.append(f"Performer: {', '.join(performers)}")
    if track_titles:
        lines.append(f"Representative tracks: {', '.join(track_titles)}")
    return "\n".join(lines)


def discover_album_candidates(paths, output_name, force):
    candidates = []
    for root in paths:
        if not root.exists() or not root.is_dir():
            eprint(f"Skipping {root}: not a directory")
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            directory = Path(dirpath)
            tracks = sorted(
                directory / filename
                for filename in filenames
                if Path(filename).suffix.lower() in AUDIO_EXTENSIONS
            )
            if not tracks:
                continue
            if find_album_art(directory) is not None:
                continue
            output_path = directory / output_name
            if output_path.exists() and not force:
                eprint(f"Skipping {directory}: {output_path.name} exists; pass --force")
                continue
            candidates.append(
                AlbumCandidate(
                    directory=directory,
                    output_path=output_path,
                    short_content=build_short_content(directory, tracks),
                )
            )
    return candidates


def run_direct_prompt(args):
    prompt = args.prompt
    if args.expand_direct_prompt:
        expanded = expand_prompt(args, f"Album: {prompt}\nArtist: Unknown Artist")
        if not expanded["ok"]:
            json_exit({"ok": False, "error": expanded["error"]}, 1)
        prompt = expanded["prompt"]

    result = generate_image(args, prompt)
    saved_file = None
    if result["ok"] and args.save:
        try:
            if write_image_result(result, Path(args.save)):
                saved_file = str(Path(args.save).resolve())
            else:
                result = {
                    "ok": False,
                    "error": {"code": "NO_IMAGE", "message": "image response had no URL or b64_json"},
                }
        except Exception as error:
            result = {"ok": False, "error": {"code": "DOWNLOAD_FAILED", "message": str(error)}}

    json_exit(
        {
            "ok": result["ok"],
            "model": args.model,
            "image_url": result.get("image_url"),
            "prompt": result.get("prompt"),
            "size": result.get("size"),
            "quality": result.get("quality"),
            "created": result.get("created"),
            "content_filter": result.get("content_filter", []),
            "saved_file": saved_file,
            "error": result.get("error"),
        },
        0 if result["ok"] else 1,
    )


def run_album_mode(args):
    candidates = discover_album_candidates(args.paths, args.output_name, args.force)
    if args.limit is not None:
        candidates = candidates[: args.limit]

    generated = 0
    failed = 0
    results = []

    eprint(f"Found {len(candidates)} missing-art album candidates")
    for index, candidate in enumerate(candidates, start=1):
        eprint(f"[{index}/{len(candidates)}] {candidate.directory}")
        if args.verbose or args.dry_run:
            eprint(candidate.short_content)

        if args.prompt:
            prompt = args.prompt
        else:
            expanded = expand_prompt(args, candidate.short_content)
            if not expanded["ok"]:
                failed += 1
                results.append(
                    {
                        "album": str(candidate.directory),
                        "ok": False,
                        "error": expanded["error"],
                    }
                )
                eprint(f"  Prompt generation failed: {expanded['error']}")
                continue
            prompt = expanded["prompt"]

        eprint(f"  Prompt: {prompt}")
        if args.dry_run:
            results.append(
                {
                    "album": str(candidate.directory),
                    "ok": True,
                    "dry_run": True,
                    "prompt": prompt,
                    "output_path": str(candidate.output_path),
                }
            )
            continue

        result = generate_image(args, prompt)
        if not result["ok"]:
            failed += 1
            results.append(
                {
                    "album": str(candidate.directory),
                    "ok": False,
                    "prompt": prompt,
                    "error": result["error"],
                }
            )
            eprint(f"  Image generation failed: {result['error']}")
            continue

        try:
            write_image_result(result, candidate.output_path)
        except Exception as error:
            failed += 1
            results.append(
                {
                    "album": str(candidate.directory),
                    "ok": False,
                    "prompt": prompt,
                    "error": {"code": "WRITE_FAILED", "message": str(error)},
                }
            )
            eprint(f"  Write failed: {error}")
            continue

        generated += 1
        results.append(
            {
                "album": str(candidate.directory),
                "ok": True,
                "prompt": prompt,
                "output_path": str(candidate.output_path),
            }
        )
        eprint(f"  Wrote {candidate.output_path}")

    json_exit(
        {
            "ok": failed == 0,
            "processed": len(candidates),
            "generated": generated,
            "failed": failed,
            "results": results,
        },
        0 if failed == 0 else 1,
    )


def main():
    args = parse_args()

    if not args.prompt and not args.paths:
        json_exit(
            {
                "ok": False,
                "error": {"code": "NO_INPUT", "message": "pass --prompt or one or more paths"},
            },
            2,
        )

    if not validate_size(args.size, args.model):
        json_exit(
            {
                "ok": False,
                "error": {
                    "code": "INVALID_SIZE",
                    "message": f"invalid size {args.size} for model {args.model}",
                },
            },
            1,
        )
    if args.user_id and not validate_user_id(args.user_id):
        json_exit(
            {
                "ok": False,
                "error": {
                    "code": "INVALID_USER_ID",
                    "message": "user_id must be 6 to 128 characters",
                },
            },
            1,
        )

    if args.paths:
        run_album_mode(args)
    else:
        run_direct_prompt(args)


if __name__ == "__main__":
    main()

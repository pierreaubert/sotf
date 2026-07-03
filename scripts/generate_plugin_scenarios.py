#!/usr/bin/env python3
"""Generate one sotf-dev-driver scenario per factory plugin type."""
import argparse
import os
import requests
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCENARIOS_DIR = ROOT / "crates" / "sotf-dev-driver" / "scenarios" / "plugins"

PLUGIN_TYPES = [
    "Gain", "EQ", "Compressor", "Limiter", "Gate", "Expander",
    "MultibandCompressor", "MultibandExpander", "LoudnessCompensation",
    "FletcherMunson", "Upmixer", "AAE", "BinauralDecoder", "Convolution",
    "LoudnessMonitor", "SpectrumAnalyzer", "ChannelMuteSolo", "Matrix",
    "XTC", "Denoiser", "Declick", "HissReducer", "SpeechDenoiser", "Pnd",
    "ABCompare", "Crossover", "BandSplit", "BandMerge", "Downmix",
    "MonoToStereo", "Crossfeed", "Delay", "Aec", "Beamformer",
    "AmbisonicsDecoder", "StereoImager", "DeEsser", "TransientShaper",
    "Saturation", "DynamicEq", "FirDesigner", "LinearPhaseEq", "SpectralCompressor",
]


def query(base: str, path: str):
    r = requests.get(f"{base}/query?path={path}", timeout=5)
    r.raise_for_status()
    j = r.json()
    if not j.get("ok"):
        raise RuntimeError(j.get("error"))
    return j["value"]


def query_or_none(base: str, path: str):
    """Query the dev API and return None if the path is invalid/out of range."""
    try:
        return query(base, path)
    except (requests.exceptions.HTTPError, RuntimeError):
        return None


def action(base: str, name: str, payload: dict):
    r = requests.post(f"{base}/action", json={"name": name, "payload": payload}, timeout=5)
    r.raise_for_status()
    j = r.json()
    if not j.get("ok"):
        raise RuntimeError(j.get("error"))


def plugin_list(base: str) -> list[dict]:
    return query(base, "plugins.list")


def find_insert_index(base: str) -> int:
    """Add and remove a probe plugin to discover the user-plugin insert index.

    The user-plugin rack is cleared first so pre-existing plugins do not shift
    the detected index.
    """
    probe_type = "Gain"
    action(base, "PluginClear", {})
    before = {entry["index"]: entry for entry in plugin_list(base)}
    action(base, "PluginAdd", {"plugin_type": probe_type})
    after = plugin_list(base)

    new_index = None
    for entry in after:
        idx = entry["index"]
        if idx not in before or before[idx] != entry:
            new_index = idx
            break
    if new_index is None:
        raise RuntimeError("could not determine user-plugin insert index")

    action(base, "PluginRemove", {"index": new_index})
    return new_index


def pick_value(param: dict) -> float:
    t = param["type"]
    if t in ("float", "int"):
        return param["min"] + (param["max"] - param["min"]) * 0.3
    if t == "bool":
        return 1.0
    if t == "choice":
        return 1.0 if param["choice_count"] > 1 else 0.0
    return 0.0


def generate_for_type(base: str, plugin_type: str, insert_idx: int) -> str:
    action(base, "PluginClear", {})
    action(base, "PluginAdd", {"plugin_type": plugin_type})
    param_count = int(query(base, f"plugins.plugin.{insert_idx}.param_count"))

    lines = [
        f"# {plugin_type} plugin lifecycle scenario",
        "focus plugins",
        "plugin_clear",
        f"plugin_add {plugin_type}",
        "assert plugins.count > 0",
        f'assert plugins.plugin.{insert_idx}.type == "{plugin_type}"',
        "",
        f"# {param_count} parameter(s)",
    ]

    for i in range(param_count):
        name = query_or_none(base, f"plugins.plugin.{insert_idx}.param.{i}.name")
        if name is None:
            lines.append(f"# stopping at parameter {i}: query out of range")
            break
        typ = query(base, f"plugins.plugin.{insert_idx}.param.{i}.type")
        meta = {"type": typ, "name": name}
        if typ == "file":
            lines.append(f"# skipping file parameter {i}: {name}")
            continue
        if typ in ("float", "int"):
            meta["min"] = query(base, f"plugins.plugin.{insert_idx}.param.{i}.min")
            meta["max"] = query(base, f"plugins.plugin.{insert_idx}.param.{i}.max")
        if typ == "choice":
            meta["choice_count"] = query(base, f"plugins.plugin.{insert_idx}.param.{i}.choice_count")
        value = pick_value(meta)
        lines.append(f"plugin_param_set {insert_idx} {i} {value}")

    lines.extend([
        "",
        "# Save, remove, reload",
        f"plugin_chain_save $SOTF_QA_DIR/{plugin_type}.json",
        f"plugin_remove {insert_idx}",
        f'assert plugins.plugin.{insert_idx}.type != "{plugin_type}"',
        f"plugin_chain_load $SOTF_QA_DIR/{plugin_type}.json",
        f'assert plugins.plugin.{insert_idx}.type == "{plugin_type}"',
        "",
        "# Delete and verify cleanup",
        f"plugin_remove {insert_idx}",
        f'assert plugins.plugin.{insert_idx}.type != "{plugin_type}"',
    ])

    action(base, "PluginRemove", {"index": insert_idx})
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default=os.environ.get("SOTF_DEV_API_URL", "http://127.0.0.1:7777"))
    args = parser.parse_args()

    SCENARIOS_DIR.mkdir(parents=True, exist_ok=True)
    insert_idx = find_insert_index(args.url)
    print(f"# user-plugin insert index: {insert_idx}")
    for pt in PLUGIN_TYPES:
        content = generate_for_type(args.url, pt, insert_idx)
        (SCENARIOS_DIR / f"{pt}.scn").write_text(content)
        print(f"generated {pt}.scn")


if __name__ == "__main__":
    main()

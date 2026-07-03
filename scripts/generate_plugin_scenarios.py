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


def action(base: str, name: str, payload: dict):
    r = requests.post(f"{base}/action", json={"name": name, "payload": payload}, timeout=5)
    r.raise_for_status()
    j = r.json()
    if not j.get("ok"):
        raise RuntimeError(j.get("error"))


def pick_value(param: dict) -> float:
    t = param["type"]
    if t in ("float", "int"):
        return param["min"] + (param["max"] - param["min"]) * 0.3
    if t == "bool":
        return 1.0
    if t == "choice":
        return 1.0 if param["choice_count"] > 1 else 0.0
    return 0.0


def generate_for_type(base: str, plugin_type: str) -> str:
    action(base, "PluginAdd", {"plugin_type": plugin_type})
    param_count = int(query(base, "plugins.plugin.0.param_count"))

    lines = [
        f"# {plugin_type} plugin lifecycle scenario",
        "focus plugins",
        f"plugin_add {plugin_type}",
        "assert plugins.count > 0",
        f'assert plugins.plugin.0.type == "{plugin_type}"',
        "",
        f"# {param_count} parameter(s)",
    ]

    for i in range(param_count):
        name = query(base, f"plugins.plugin.0.param.{i}.name")
        typ = query(base, f"plugins.plugin.0.param.{i}.type")
        meta = {"type": typ, "name": name}
        if typ == "file":
            lines.append(f"# skipping file parameter {i}: {name}")
            continue
        if typ in ("float", "int"):
            meta["min"] = query(base, f"plugins.plugin.0.param.{i}.min")
            meta["max"] = query(base, f"plugins.plugin.0.param.{i}.max")
        if typ == "choice":
            meta["choice_count"] = query(base, f"plugins.plugin.0.param.{i}.choice_count")
        value = pick_value(meta)
        lines.append(f"plugin_param_set 0 {i} {value}")

    lines.extend([
        "",
        "# Save, remove, reload",
        f"plugin_chain_save $SOTF_QA_DIR/{plugin_type}.json",
        "plugin_remove 0",
        "assert plugins.count == 0",
        f"plugin_chain_load $SOTF_QA_DIR/{plugin_type}.json",
        f'assert plugins.plugin.0.type == "{plugin_type}"',
        "",
        "# Delete and verify cleanup",
        "plugin_remove 0",
        "assert plugins.count == 0",
    ])

    action(base, "PluginRemove", {"index": 0})
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default=os.environ.get("SOTF_DEV_API_URL", "http://127.0.0.1:7777"))
    args = parser.parse_args()

    SCENARIOS_DIR.mkdir(parents=True, exist_ok=True)
    for pt in PLUGIN_TYPES:
        content = generate_for_type(args.url, pt)
        (SCENARIOS_DIR / f"{pt}.scn").write_text(content)
        print(f"generated {pt}.scn")


if __name__ == "__main__":
    main()

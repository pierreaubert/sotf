#!/usr/bin/env python3
"""Slim measured RoomEQ DSP artifacts into sotf test fixtures.

Source: ../autoeq/data_generated/roomeq/measured/<dataset>/<mode>/dsp-<mode>.json
(plus dsp-<mode>_files/<CH>__eq.csv). Those artifacts are generated and not
git-tracked, so the test fixtures copy only what the DSP execution test
consumes, following the roomeq_gen514_dsp.json precedent:

- dsp.json: per-channel plugin chains and routing metadata verbatim; large
  curve arrays (freq/spl style payloads and any numeric list over 64 entries
  outside `plugins` / `routing_graph`) are emptied. metadata is reduced to
  scores/algorithm/playback_summary/home_cinema_layout/bass_management.
- <CH>__eq.csv: stride-downsampled to at most ~160 rows (exact source rows,
  no interpolation), used as the response oracle.

Run from the sotf repo root. Re-running after regenerating the autoeq
artifacts refreshes the fixtures deterministically.
"""

import json
import math
import shutil
from pathlib import Path

SRC = Path("../autoeq/data_generated/roomeq/measured")
DST = Path("crates/sotf-player/tests/fixtures/roomeq_measured")
DATASETS = ["2.0_t7v_2026", "5.1.4_genelec"]
MODES = ["iir", "fir", "mixed", "mixed-phase"]
KEEP_METADATA = {
    "pre_score",
    "post_score",
    "algorithm",
    "loss_type",
    "iterations",
    "timestamp",
    "playback_summary",
    "home_cinema_layout",
    "bass_management",
}
MAX_EQ_ROWS = 160


def slim(value, in_plugins=False):
    if isinstance(value, dict):
        keys = set(value)
        if not in_plugins and ({"freq", "spl"} <= keys or {"frequencies", "spl_db"} <= keys):
            return {k: ([] if isinstance(v, list) else slim(v)) for k, v in value.items()}
        return {k: slim(v, in_plugins or k == "plugins") for k, v in value.items()}
    if isinstance(value, list):
        if not in_plugins and len(value) > 64 and all(
            isinstance(item, (int, float)) for item in value
        ):
            return []
        return [slim(item, in_plugins) for item in value]
    return value


def slim_chain(source: Path, dest: Path) -> None:
    data = json.loads(source.read_text())
    out = {
        "version": data["version"],
        "global_plugins": data.get("global_plugins", []),
        "channels": {
            name: {
                "channel": chain["channel"],
                "plugins": chain.get("plugins", []),
                **(
                    {"drivers": chain["drivers"]}
                    if chain.get("drivers") is not None
                    else {}
                ),
            }
            for name, chain in data["channels"].items()
        },
    }
    metadata = data.get("metadata") or {}
    kept = {
        key: slim(metadata[key]) for key in KEEP_METADATA if metadata.get(key) is not None
    }
    if kept:
        out["metadata"] = kept
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(out, indent=1) + "\n")


def slim_eq_csv(source: Path, dest: Path) -> None:
    lines = source.read_text().splitlines()
    header, rows = lines[0], lines[1:]
    stride = max(1, math.ceil(len(rows) / MAX_EQ_ROWS))
    picked = rows[::stride]
    if rows and picked[-1] != rows[-1]:
        picked.append(rows[-1])
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text("\n".join([header, *picked]) + "\n")


def main() -> None:
    if DST.exists():
        shutil.rmtree(DST)
    for dataset in DATASETS:
        for mode in MODES:
            mode_dir = SRC / dataset / mode
            chain = mode_dir / f"dsp-{mode}.json"
            out_dir = DST / dataset
            if not chain.exists():
                diagnostic = mode_dir / "diagnostic-artifact.json"
                assert diagnostic.exists(), f"{dataset}/{mode}: no chain and no diagnostic"
                out_dir.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(diagnostic, out_dir / f"{mode}-diagnostic.json")
                print(f"{dataset}/{mode}: rejected upstream, diagnostic copied")
                continue
            slim_chain(chain, out_dir / f"dsp-{mode}.json")
            files_dir = mode_dir / f"dsp-{mode}_files"
            eq_dir = out_dir / f"{mode}-eq"
            for eq_csv in sorted(files_dir.glob("*__eq.csv")):
                slim_eq_csv(eq_csv, eq_dir / eq_csv.name)
            print(f"{dataset}/{mode}: slimmed chain + eq curves")


if __name__ == "__main__":
    main()

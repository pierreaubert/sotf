"""Check the GNU/Linux AppImage CPU contract without linking a Linux binary.

Run with ./venv/bin/python scripts/check-linux-cpu-baseline.py.
"""

import pathlib
import subprocess
import tomllib


def main() -> None:
    root = pathlib.Path(__file__).resolve().parent.parent
    config = tomllib.loads((root / ".cargo/config.toml").read_text())
    target = "x86_64-unknown-linux-gnu"
    flags = config["target"][target].get("rustflags", config["build"]["rustflags"])
    # Compare with rustc's own baseline, including implied features. No target
    # standard library or Linux linker is needed for --print cfg.
    command = ["rustc", "--target", target, "--print", "cfg"]

    def features(arguments: list[str]) -> set[str]:
        result = subprocess.run(
            command + arguments, cwd=root, check=True, capture_output=True, text=True
        )
        return {
            line for line in result.stdout.splitlines()
            if line.startswith('target_feature=')
        }

    baseline = features(["-C", "target-cpu=x86-64-v2"])
    configured = features(flags)
    extra = configured - baseline
    if extra:
        raise SystemExit("Linux build requires unsupported CPU features: " + ", ".join(sorted(extra)))
    print("GNU/Linux configured CPU features stay within x86-64-v2")


if __name__ == "__main__":
    main()

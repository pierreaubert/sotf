#!/usr/bin/env python3
"""Run Cargo with PipeWire development headers available on Linux.

The native PipeWire backend is enabled by the shipping CLI/TUI/GPUI
feature sets. Its `libspa-sys` build script requires
`libpipewire-0.3` from pkg-config. Installed system headers are
preferred. On Debian/Ubuntu hosts without them, this wrapper
provisioning path extracts the two development packages into an
ignored local sysroot (no sudo) and points pkg-config at it. Set
`SOTF_DISABLE_PIPEWIRE_SYSROOT=1` to require system headers only.
"""

from __future__ import annotations

import os
from pathlib import Path
import platform
import subprocess
import sys
from typing import Callable, Mapping, Sequence


PACKAGES = ("libpipewire-0.3-dev", "libspa-0.2-dev")
REMEDY = (
    "install libpipewire-0.3-dev and libspa-0.2-dev "
    "(for example with `just install-ubuntu-common`), or set "
    "SOTF_PIPEWIRE_SYSROOT to an extracted development sysroot"
)


class ProvisioningError(RuntimeError):
    """Raised when local PipeWire headers cannot be made available."""


Runner = Callable[..., subprocess.CompletedProcess]
BUILD_COMMANDS = {"build", "check", "clippy", "llvm-cov", "nextest", "run", "rustc", "test"}


def command_needs_pipewire(command: Sequence[str]) -> bool:
    """Return whether a wrapped Cargo command can compile native code."""
    return any(argument in BUILD_COMMANDS for argument in command)


def _pkg_config_exists(env: Mapping[str, str], run: Runner) -> bool:
    try:
        result = run(
            ["pkg-config", "--exists", "libpipewire-0.3"],
            env=dict(env),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    except (FileNotFoundError, OSError):
        return False
    return result.returncode == 0


def find_pkgconfig_dir(sysroot: Path) -> Path | None:
    """Return the directory containing libpipewire's pkg-config file."""
    patterns = (
        "usr/lib/*/pkgconfig/libpipewire-0.3.pc",
        "usr/lib/pkgconfig/libpipewire-0.3.pc",
        "usr/share/pkgconfig/libpipewire-0.3.pc",
        "usr/local/lib/*/pkgconfig/libpipewire-0.3.pc",
    )
    for pattern in patterns:
        matches = sorted(sysroot.glob(pattern))
        if matches:
            return matches[0].parent
    return None


def _default_sysroot(root: Path, env: Mapping[str, str]) -> Path:
    explicit = env.get("SOTF_PIPEWIRE_SYSROOT")
    if explicit:
        return Path(explicit).expanduser()
    target_dir = env.get("CARGO_TARGET_DIR")
    if target_dir:
        target_path = Path(target_dir).expanduser()
        if not target_path.is_absolute():
            target_path = root / target_path
    else:
        target_path = root / "target"
    return target_path / "pipewire-dev/root"


def candidate_sysroots(root: Path, env: Mapping[str, str]) -> list[Path]:
    """Return local sysroots to try, in preference order."""
    candidates: list[Path] = []
    for key in ("SOTF_PIPEWIRE_SYSROOT", "PKG_CONFIG_SYSROOT_DIR"):
        value = env.get(key)
        if value:
            candidates.append(Path(value).expanduser())
    candidates.append(_default_sysroot(root, env))
    # Sysroot provisioned by the aggregate release gate before this
    # helper existed.
    candidates.append(root / "tmp/pipewire-dev/root")

    unique: list[Path] = []
    for candidate in candidates:
        if candidate not in unique:
            unique.append(candidate)
    return unique


def _host_triplet(run: Runner) -> str:
    for argv in (
        ["dpkg-architecture", "-q", "DEB_HOST_MULTIARCH"],
        ["gcc", "-dumpmachine"],
    ):
        try:
            result = run(argv, capture_output=True, text=True, check=False)
        except (FileNotFoundError, OSError):
            continue
        if result.returncode == 0 and result.stdout and result.stdout.strip():
            return result.stdout.strip()
    return f"{platform.machine()}-linux-gnu"


def _fix_runtime_link(
    sysroot: Path,
    triplet: str,
    system_lib_root: Path = Path("/usr/lib"),
) -> None:
    """Point the extracted dev symlink at the installed runtime library."""
    library_dir = sysroot / "usr/lib" / triplet
    dev_link = library_dir / "libpipewire-0.3.so"
    if not dev_link.is_symlink():
        return
    try:
        target_name = os.readlink(dev_link)
    except OSError:
        return
    target = Path(target_name)
    if not target.is_absolute():
        target = library_dir / target
    if target.exists():
        return
    system_libraries = sorted(
        (system_lib_root / triplet).glob("libpipewire-0.3.so.0*")
    )
    if not system_libraries:
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    target.symlink_to(system_libraries[0])


def provision_sysroot(
    sysroot: Path,
    run: Runner = subprocess.run,
    system_lib_root: Path = Path("/usr/lib"),
) -> None:
    """Download and extract Debian/Ubuntu PipeWire dev packages."""
    debs_dir = sysroot.parent / "debs"
    debs_dir.mkdir(parents=True, exist_ok=True)
    sysroot.mkdir(parents=True, exist_ok=True)
    try:
        result = run(
            ["apt-get", "download", *PACKAGES],
            cwd=debs_dir,
            capture_output=True,
            text=True,
            check=False,
        )
    except (FileNotFoundError, OSError) as error:
        raise ProvisioningError(
            f"PipeWire development headers are missing and apt-get is "
            f"unavailable ({error}); {REMEDY}"
        ) from error
    if result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip()
        raise ProvisioningError(
            "PipeWire development headers are missing and downloading "
            f"them failed ({detail or 'apt-get download failed'}); {REMEDY}"
        )

    packages = sorted(debs_dir.glob("*.deb"))
    if not packages:
        raise ProvisioningError(
            f"PipeWire development packages were not downloaded; {REMEDY}"
        )
    for package in packages:
        try:
            result = run(
                ["dpkg-deb", "-x", str(package), str(sysroot)],
                capture_output=True,
                text=True,
                check=False,
            )
        except (FileNotFoundError, OSError) as error:
            raise ProvisioningError(
                f"could not extract {package.name} ({error}); {REMEDY}"
            ) from error
        if result.returncode != 0:
            detail = (result.stderr or result.stdout or "").strip()
            raise ProvisioningError(
                f"could not extract {package.name} ({detail}); {REMEDY}"
            )

    _fix_runtime_link(sysroot, _host_triplet(run), system_lib_root)
    if find_pkgconfig_dir(sysroot) is None:
        raise ProvisioningError(
            f"provisioned PipeWire sysroot has no pkg-config metadata; {REMEDY}"
        )


def cargo_environment(
    root: Path,
    env: Mapping[str, str] | None = None,
    *,
    platform_name: str | None = None,
    run: Runner = subprocess.run,
    system_lib_root: Path = Path("/usr/lib"),
) -> dict[str, str]:
    """Return Cargo's environment, adding local PipeWire headers if needed."""
    result = dict(os.environ if env is None else env)
    if not (platform_name or sys.platform).startswith("linux"):
        return result
    target = result.get("CARGO_BUILD_TARGET", "")
    if target and "linux-gnu" not in target:
        return result
    if _pkg_config_exists(result, run):
        return result
    if result.get("SOTF_DISABLE_PIPEWIRE_SYSROOT", "").lower() in {
        "1",
        "true",
        "yes",
    }:
        raise ProvisioningError(f"PipeWire development headers are missing; {REMEDY}")

    selected: Path | None = None
    pkgconfig_dir: Path | None = None
    for candidate in candidate_sysroots(root, result):
        pkgconfig_dir = find_pkgconfig_dir(candidate)
        if pkgconfig_dir is not None:
            selected = candidate
            break
    if selected is None:
        selected = _default_sysroot(root, result)
        provision_sysroot(selected, run=run, system_lib_root=system_lib_root)
        pkgconfig_dir = find_pkgconfig_dir(selected)
    if pkgconfig_dir is None:
        raise ProvisioningError(f"PipeWire development headers are missing; {REMEDY}")

    result["PKG_CONFIG_SYSROOT_DIR"] = str(selected)
    existing = result.get("PKG_CONFIG_PATH", "")
    paths = existing.split(os.pathsep) if existing else []
    if str(pkgconfig_dir) not in paths:
        result["PKG_CONFIG_PATH"] = os.pathsep.join([str(pkgconfig_dir), *paths])
    if not _pkg_config_exists(result, run):
        raise ProvisioningError(
            f"PipeWire headers under {selected} are not usable by pkg-config; "
            f"{REMEDY}"
        )
    return result


def main(argv: Sequence[str] | None = None) -> int:
    command = list(sys.argv[1:] if argv is None else argv)
    if not command:
        print(
            "usage: cargo_with_pipewire_sysroot.py COMMAND [ARGUMENTS...]",
            file=sys.stderr,
        )
        return 2
    root = Path(__file__).resolve().parents[1]
    try:
        environment = (
            cargo_environment(root)
            if command_needs_pipewire(command)
            else dict(os.environ)
        )
    except ProvisioningError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    sysroot = environment.get("PKG_CONFIG_SYSROOT_DIR")
    if sysroot and sysroot != os.environ.get("PKG_CONFIG_SYSROOT_DIR"):
        print(
            f"Using local PipeWire sysroot at {sysroot}; {REMEDY} "
            "to use system headers instead.",
            file=sys.stderr,
        )
    # Replace this process so Cargo keeps native signal/exit behavior.
    os.execvpe(command[0], command, environment)
    return 127


if __name__ == "__main__":
    raise SystemExit(main())

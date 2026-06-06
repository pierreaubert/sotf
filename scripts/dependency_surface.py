#!/usr/bin/env python3
"""Report dependency surface and supply-chain roots from cargo metadata."""

from __future__ import annotations

import argparse
import collections
import json
import subprocess
import sys


def source_kind(package: dict) -> str:
    source = package.get("source")
    if source is None:
        return "path"
    if str(source).startswith("git+"):
        return "git"
    return "registry"


def dependency_is_included(dep: dict, include_dev: bool) -> bool:
    if include_dev:
        return True
    dep_kinds = dep.get("dep_kinds") or []
    return any(kind.get("kind") != "dev" for kind in dep_kinds)


def closure(root_id: str, nodes: dict, include_dev: bool) -> set[str]:
    seen: set[str] = set()
    stack = [root_id]
    while stack:
        pkg_id = stack.pop()
        if pkg_id in seen:
            continue
        seen.add(pkg_id)
        for dep in nodes.get(pkg_id, {}).get("deps", []):
            if dependency_is_included(dep, include_dev):
                stack.append(dep["pkg"])
    return seen


def run_metadata(cargo_args: list[str]) -> dict:
    cmd = ["cargo", "metadata", "--format-version", "1", *cargo_args]
    try:
        return json.loads(subprocess.check_output(cmd, text=True))
    except subprocess.CalledProcessError as exc:
        print(f"failed to run {' '.join(cmd)}", file=sys.stderr)
        raise SystemExit(exc.returncode) from exc


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "-p",
        "--package",
        default="sotf-gpui",
        help="workspace package to report, default: sotf-gpui",
    )
    parser.add_argument(
        "--include-dev",
        action="store_true",
        help="include dev-dependency edges in closure calculations",
    )
    parser.add_argument(
        "cargo_args",
        nargs=argparse.REMAINDER,
        help="extra args passed to cargo metadata after --, e.g. -- --no-default-features",
    )
    args = parser.parse_args()

    cargo_args = args.cargo_args[1:] if args.cargo_args[:1] == ["--"] else args.cargo_args
    metadata = run_metadata(cargo_args)
    packages = {pkg["id"]: pkg for pkg in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}

    roots = [pkg_id for pkg_id, pkg in packages.items() if pkg["name"] == args.package]
    if not roots:
        print(f"package not found in cargo metadata: {args.package}", file=sys.stderr)
        return 2
    root_id = roots[0]
    root_pkg = packages[root_id]
    selected = closure(root_id, nodes, args.include_dev)
    selected_pkgs = [packages[pkg_id] for pkg_id in selected]

    source_counts = collections.Counter(source_kind(pkg) for pkg in selected_pkgs)
    versions: dict[str, set[str]] = collections.defaultdict(set)
    for pkg in selected_pkgs:
        if source_kind(pkg) != "path":
            versions[pkg["name"]].add(pkg["version"])
    duplicate_versions = sorted(
        (name, sorted(vals)) for name, vals in versions.items() if len(vals) > 1
    )

    direct_rows = []
    for dep in nodes[root_id].get("deps", []):
        if not dependency_is_included(dep, args.include_dev):
            continue
        dep_id = dep["pkg"]
        dep_closure = closure(dep_id, nodes, args.include_dev)
        dep_pkgs = [packages[pkg_id] for pkg_id in dep_closure]
        dep_counts = collections.Counter(source_kind(pkg) for pkg in dep_pkgs)
        direct_rows.append(
            (
                dep_counts["registry"] + dep_counts["git"],
                dep_counts["git"],
                dep_counts["registry"],
                dep_counts["path"],
                packages[dep_id]["name"],
                source_kind(packages[dep_id]),
            )
        )

    git_packages = sorted(
        (pkg["name"], pkg["version"], pkg["source"])
        for pkg in selected_pkgs
        if source_kind(pkg) == "git"
    )

    print(f"# Dependency Surface: {root_pkg['name']} {root_pkg['version']}")
    if cargo_args:
        print(f"cargo metadata args: {' '.join(cargo_args)}")
    print()
    print("## Resolved Closure")
    print(f"- total packages: {len(selected_pkgs)}")
    print(f"- path packages: {source_counts['path']}")
    print(f"- registry packages: {source_counts['registry']}")
    print(f"- git packages: {source_counts['git']}")
    print()
    print("## Largest Direct Roots")
    print("| external | git | registry | path | direct dep | source |")
    print("|---:|---:|---:|---:|---|---|")
    for row in sorted(direct_rows, reverse=True)[:30]:
        external, git, registry, path, name, kind = row
        print(f"| {external} | {git} | {registry} | {path} | {name} | {kind} |")
    print()
    print("## Git Packages")
    if git_packages:
        for name, version, source in git_packages:
            print(f"- {name} {version}: {source}")
    else:
        print("- none")
    print()
    print("## Duplicate External Versions")
    if duplicate_versions:
        for name, vals in duplicate_versions[:80]:
            print(f"- {name}: {', '.join(vals)}")
    else:
        print("- none")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

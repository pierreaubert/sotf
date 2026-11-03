"""Verify macOS helper build and bundle contracts without native build tools."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parent / "lib/macos_plugin_helpers.sh"
HELPERS = ("sotf-external-plugin-worker", "sotf-macos-sandbox-helper")


class MacosPluginHelpersTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="sotf-plugin-helpers-")
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.tools = self.base / "tools"
        self.tools.mkdir()
        self.source = self.base / "source with spaces"
        self.source.mkdir()
        self.destination = self.base / "Desktop.app/Contents/MacOS"
        self.desktop = self.source / "sotf-desktop"
        self.desktop.write_text("arm64\n")
        for name in HELPERS:
            self.helper(name, "arm64")
        self.env = dict(os.environ, PATH=str(self.tools) + os.pathsep + os.environ["PATH"])
        self.tool("lipo", "import pathlib,sys\n"
                  "args=sys.argv[1:]\n"
                  "arches=pathlib.Path(args[-1]).read_text().splitlines()[0].split()\n"
                  "if args[0]=='-archs': print(' '.join(arches))\n"
                  "else: sys.exit(0 if args[1] in arches else 1)\n")

    def tool(self, name, body):
        path = self.tools / name
        path.write_text("#!" + sys.executable + "\n" + body)
        path.chmod(0o755)

    def helper(self, name, arches):
        path = self.source / name
        path.write_text(arches + "\n" + name + " payload\n")
        path.chmod(0o755)

    def invoke(self, function, *arguments):
        return subprocess.run(
            ["/bin/bash", "-c", 'source "$1"; shift; ' + function + ' "$@"',
             "helper-test", str(SCRIPT), *map(str, arguments)],
            env=self.env, text=True, capture_output=True,
        )

    def stage(self):
        return self.invoke("stage_macos_plugin_helpers", self.source, self.destination, self.desktop)

    def test_both_helpers_keep_bytes_and_executable_permissions(self):
        result = self.stage()
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in HELPERS:
            self.assertEqual((self.destination / name).read_bytes(), (self.source / name).read_bytes())
            self.assertTrue(os.access(self.destination / name, os.X_OK))

    def test_missing_helper_rejects_entire_pair(self):
        (self.source / HELPERS[1]).unlink()
        result = self.stage()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(HELPERS[1], result.stderr)
        self.assertFalse(self.destination.exists())

    def test_wrong_architecture_rejects_entire_pair(self):
        self.helper(HELPERS[1], "x86_64")
        result = self.stage()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("arm64", result.stderr)
        self.assertFalse(self.destination.exists())

    def test_universal_desktop_requires_each_helper_architecture(self):
        self.desktop.write_text("arm64 x86_64\n")
        result = self.stage()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("x86_64", result.stderr)
        self.assertFalse(self.destination.exists())

    def test_build_uses_daw_manifest_and_au_feature(self):
        log = self.base / "cargo.json"
        self.env["SOTF_TEST_CARGO_LOG"] = str(log)
        self.tool("cargo", "import json,os,pathlib,sys\n"
                  "pathlib.Path(os.environ['SOTF_TEST_CARGO_LOG']).write_text(json.dumps(sys.argv[1:]))\n")
        daw = self.base / "sotf-daw"
        target_dir = self.base / "shared target"
        result = self.invoke("build_macos_plugin_helpers", daw, target_dir, "aarch64-apple-darwin")
        self.assertEqual(result.returncode, 0, result.stderr)
        args = json.loads(log.read_text())
        self.assertEqual(args[args.index("--manifest-path") + 1], str(daw / "Cargo.toml"))
        self.assertEqual(args[args.index("--target-dir") + 1], str(target_dir))
        self.assertEqual(args[args.index("--features") + 1], "external-plugin-au")
        self.assertEqual(args[args.index("--target") + 1], "aarch64-apple-darwin")
        self.assertNotIn("--locked", args)
        self.assertEqual([args[i + 1] for i, arg in enumerate(args) if arg == "--bin"], list(HELPERS))


if __name__ == "__main__":
    unittest.main()

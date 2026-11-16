"""Regression tests for the Linux PipeWire header fallback used by Just."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import cargo_with_pipewire_sysroot as helper


class FakeRunner:
    """Small subprocess stand-in for pkg-config/apt provisioning tests."""

    def __init__(self):
        self.calls = []

    def __call__(self, argv, **kwargs):
        self.calls.append((list(argv), kwargs))
        env = kwargs.get("env") or {}
        if argv[:2] == ["pkg-config", "--exists"]:
            sysroot = env.get("PKG_CONFIG_SYSROOT_DIR")
            if sysroot and list((Path(sysroot) / "usr/lib").glob(
                "*/pkgconfig/libpipewire-0.3.pc"
            )):
                return subprocess.CompletedProcess(argv, 0)
            return subprocess.CompletedProcess(argv, 1)
        if argv[:2] == ["dpkg-architecture", "-q"]:
            return subprocess.CompletedProcess(
                argv, 0, stdout="x86_64-linux-gnu\n"
            )
        if argv[:2] == ["apt-get", "download"]:
            cwd = Path(kwargs["cwd"])
            for package in ("libpipewire-0.3-dev", "libspa-0.2-dev"):
                (cwd / f"{package}.deb").write_bytes(b"fake deb")
            return subprocess.CompletedProcess(argv, 0)
        if argv[:2] == ["dpkg-deb", "-x"]:
            destination = Path(argv[3])
            pkgconfig = (
                destination
                / "usr/lib/x86_64-linux-gnu/pkgconfig"
            )
            pkgconfig.mkdir(parents=True, exist_ok=True)
            (pkgconfig / "libpipewire-0.3.pc").write_text("prefix=/usr\n")
            (pkgconfig / "libspa-0.2.pc").write_text("prefix=/usr\n")
            library_dir = destination / "usr/lib/x86_64-linux-gnu"
            dev_link = library_dir / "libpipewire-0.3.so"
            if not os.path.lexists(dev_link):
                dev_link.symlink_to("libpipewire-0.3.so.0")
            return subprocess.CompletedProcess(argv, 0)
        raise AssertionError(f"unexpected command: {argv!r}")


class CargoEnvironmentTests(unittest.TestCase):
    def test_system_headers_leave_environment_unchanged(self):
        env = {"PATH": "/usr/bin", "PKG_CONFIG_PATH": "/custom/pkgconfig"}

        def available(argv, **_kwargs):
            return subprocess.CompletedProcess(argv, 0)

        result = helper.cargo_environment(
            Path("/repo/sotf"),
            env,
            platform_name="linux",
            run=available,
        )

        self.assertEqual(result, env)
        self.assertIsNot(result, env)

    def test_existing_sysroot_is_selected_and_preserves_pkg_config_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sysroot = root / "target/pipewire-dev/root"
            pkgconfig = sysroot / "usr/lib/x86_64-linux-gnu/pkgconfig"
            pkgconfig.mkdir(parents=True)
            (pkgconfig / "libpipewire-0.3.pc").write_text("prefix=/usr\n")
            runner = FakeRunner()
            env = {"PATH": "/usr/bin", "PKG_CONFIG_PATH": "/custom/pkgconfig"}

            result = helper.cargo_environment(
                root,
                env,
                platform_name="linux",
                run=runner,
            )

        self.assertEqual(result["PKG_CONFIG_SYSROOT_DIR"], str(sysroot))
        self.assertEqual(
            result["PKG_CONFIG_PATH"],
            f"{pkgconfig}:{os.pathsep}/custom/pkgconfig".replace(
                f":{os.pathsep}", os.pathsep
            ),
        )
        self.assertFalse(any(call[0][:1] == ["apt-get"] for call in runner.calls))

    def test_missing_headers_provision_local_sysroot_and_runtime_link(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "sotf"
            root.mkdir()
            system_lib_dir = (
                Path(temporary)
                / "system/usr/lib/x86_64-linux-gnu"
            )
            system_lib_dir.mkdir(parents=True)
            runtime = system_lib_dir / "libpipewire-0.3.so.0.1005.0"
            runtime.write_bytes(b"runtime")
            runner = FakeRunner()

            result = helper.cargo_environment(
                root,
                {"PATH": "/usr/bin"},
                platform_name="linux",
                run=runner,
                system_lib_root=Path(temporary) / "system/usr/lib",
            )

            sysroot = Path(result["PKG_CONFIG_SYSROOT_DIR"])
            runtime_link = (
                sysroot
                / "usr/lib/x86_64-linux-gnu/libpipewire-0.3.so.0"
            )
            self.assertTrue(
                (sysroot / "usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc")
                .is_file()
            )
            self.assertEqual(runtime_link.resolve(), runtime.resolve())
            self.assertTrue(any(call[0][:1] == ["apt-get"] for call in runner.calls))

    def test_non_linux_and_musl_targets_are_unchanged(self):
        env = {"PATH": "/usr/bin"}

        def forbidden(argv, **_kwargs):
            raise AssertionError(f"unexpected command: {argv!r}")

        self.assertEqual(
            helper.cargo_environment(
                Path("/repo/sotf"),
                env,
                platform_name="darwin",
                run=forbidden,
            ),
            env,
        )
        musl_env = {**env, "CARGO_BUILD_TARGET": "x86_64-unknown-linux-musl"}
        self.assertEqual(
            helper.cargo_environment(
                Path("/repo/sotf"),
                musl_env,
                platform_name="linux",
                run=forbidden,
            ),
            musl_env,
        )


class CommandSelectionTests(unittest.TestCase):
    def test_only_compilation_commands_need_pipewire_headers(self):
        self.assertTrue(
            helper.command_needs_pipewire(["cargo", "check", "--workspace"])
        )
        self.assertTrue(
            helper.command_needs_pipewire(["mbx", "nextest", "run"])
        )
        self.assertTrue(
            helper.command_needs_pipewire(["cargo", "llvm-cov", "--workspace"])
        )
        self.assertFalse(
            helper.command_needs_pipewire(["cargo", "fmt", "--all"])
        )
        self.assertFalse(
            helper.command_needs_pipewire(["cargo", "clean"])
        )
        self.assertFalse(
            helper.command_needs_pipewire(["cargo", "--version"])
        )


class BuildDependencyDeclarationTests(unittest.TestCase):
    """Keep every canonical Linux setup path aligned on the headers."""

    def test_linux_setup_surfaces_install_pipewire_headers(self):
        root = Path(__file__).resolve().parents[1]
        surfaces = [
            root / "builds/linux.just",
            root / ".github/workflows/tui-portability.yml",
            root / ".github/workflows/testing-tiers.yml",
            root / ".github/workflows/release.yml",
            root / ".gitea/workflows/release.yml",
        ]
        for surface in surfaces:
            with self.subTest(surface=surface.relative_to(root)):
                text = surface.read_text(encoding="utf-8")
                self.assertIn("libpipewire-0.3-dev", text)
                self.assertIn("libspa-0.2-dev", text)

    def test_gnu_linux_builder_dockerfiles_install_pipewire_headers(self):
        root = Path(__file__).resolve().parents[1]
        dockerfiles = [
            root / "builds/from_linux_x86/Dockerfile.aarch64-unknown-linux-gnu",
            root / "builds/from_linux_x86/Dockerfile.base.x86_64-unknown-linux-gnu",
            root / "builds/from_linux_x86/Dockerfile.build.x86_64-unknown-linux-gnu",
            root / "builds/from_linux_x86/Dockerfile.x86_64-unknown-linux-gnu",
            root / "builds/from_macos_arm/Dockerfile.aarch64-unknown-linux-gnu",
            root / "builds/from_macos_arm/Dockerfile.base.aarch64-unknown-linux-gnu",
            root / "builds/from_macos_arm/Dockerfile.base.x86_64-unknown-linux-gnu",
            root / "builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-gnu",
            root / "builds/linux-distros/Dockerfile.debian-x86_64-gnu",
            root / "builds/systemwide/Dockerfile.test-linux-arm64",
        ]
        for dockerfile in dockerfiles:
            with self.subTest(dockerfile=dockerfile.relative_to(root)):
                text = dockerfile.read_text(encoding="utf-8")
                self.assertIn("libpipewire-0.3-dev", text)
                self.assertIn("libspa-0.2-dev", text)


if __name__ == "__main__":
    unittest.main()

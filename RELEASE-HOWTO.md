# How to make a release


In general,
```
just
```
is your friend, it will list a long list of commands to get stuff done.

## Testing

```
just ntest
just itest
```

needs to work cleanly.

## Documentation

```
just doc
```

## QA

Full qa is very slow (hours).
```
just qa

```
 You can run a minimal subset (minutes):
```
just qa-math
just qa-autoeq
just qa-roomeq-quick
just qa-roomeq-multi-measurement
just qa-plugin
```

## Building with the magic script

```
./scripts/build-release.sh --help
```
If not, then do it step by step, see below.

## Building ARM binaries

On a ARM Mac M1-M5, in order to build binaries for Linux and Windows, you need docker:

```
just docker-linux-arm64
just docker-windows-arm64
```

All the files land in `./dist`.

## Building X86 binaries

On a Linux X86 machine, you need docker:
```
just docker-linux-x86
just docker-windows-86
```
All the files land in `./dist`.

## Building Linux distro variants

To check the x86_64 Linux build against specific Docker base images:

```
just docker-linux-x86-ubuntu-24-04
just docker-linux-x86-ubuntu-26-04
just docker-linux-x86-debian-latest
just docker-linux-x86-alpine-latest
```

Run all of the distro checks with:

```
just docker-linux-x86-distros
```

The Ubuntu and Debian recipes build the AppImage, `.deb`, and tarball. The
Alpine recipe runs the TUI-only path because `.deb` packaging and AppImage
tooling are Debian/glibc-oriented. Each recipe also copies its artifacts to
`dist/linux-distros/<distro>/` so aggregate runs keep per-distro outputs.

## Building on Windows X86

In a powershell terminal:
```
.\build\build-windows.bat
```

## Building for iOS is experimental

In a terminal, to test on Apple simulator:
```
just ios-sim
```
In a terminal, to test on devices
```
just ios-device
```


## Building for tvOS is very experimental

tvOS port is really experimental, I use patched crates and you need to use the nightly compiler.

In a terminal, to test on Apple simulator:
```
rustup upgrade
rustup target add aarch64-apple-tvos-sim
just tvos-sim
```
In a terminal, to test on devices
```
rustup upgrade
rustup target add aarch64-apple-tvos-sim
just tvos-device
```

## Signing binaries

### Mac

For developping, you can self sign which just is doing for you. For publishing, you need to have proper certificate from Apple that cost ~100$/y.

You need to be able to sign and notarize.
```
export APPLE_ID="you@me.com"
export DEVELOPER_ID="Developer ID Application: You (CODE))"
```

You can generate:
```
./scripts/build-dmg-sotf.sh --sign --notarize
```

### Linux

Cosign

### Windows

Generate a msix:

```
.\scripts\build-msix.ps1 -Arch x86_64 -Sign
```

#### MSIX runtime dependency (sideload)

The MSIX is built dynamically against the MSVC C/C++ runtime — `sotf-desktop.exe`
imports `VCRUNTIME140.dll`, `MSVCP140.dll`, and the UCRT. To avoid bundling the
redistributable, `AppxManifest.xml` declares a framework dependency on
`Microsoft.VCLibs.140.00.UWPDesktop` (the Visual C++ 2015–2022 Runtime
framework package).

- **Microsoft Store install**: nothing to do; the Store resolves the framework
  dependency automatically.
- **Sideload install**: the target machine must have the VCLibs framework
  package registered before the MSIX will deploy. On most up-to-date Windows
  10/11 installs it is already present (any Store app pulls it in). On a fresh
  box, install it once per architecture:

  ```powershell
  # x64
  Invoke-WebRequest https://aka.ms/Microsoft.VCLibs.x64.14.00.Desktop.appx `
      -OutFile $env:TEMP\VCLibs.x64.appx
  Add-AppxPackage $env:TEMP\VCLibs.x64.appx

  # arm64
  Invoke-WebRequest https://aka.ms/Microsoft.VCLibs.arm64.14.00.Desktop.appx `
      -OutFile $env:TEMP\VCLibs.arm64.appx
  Add-AppxPackage $env:TEMP\VCLibs.arm64.appx
  ```

  Without VCLibs, `Add-AppxPackage sotf-desktop-*.msix` fails with
  `0x80073CF3 (PACKAGE_FAILURE_RESOLVING_DEPENDENCY)`. The error names the
  missing framework package, so it is unambiguous — no silent "exe won't
  start" failure mode.

Note: nothing else is bundled — `nlopt.dll` is gone (cobyla was rewritten in
pure Rust). The MSIX ships only `sotf-desktop.exe` plus assets.

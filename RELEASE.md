# How to make a release

## Testing

```
just ntest
```

need to work cleanly.

## QA

Full qa is very slow.
```
just qa

```
 You can run a minial subset:
```
just qa-autoeq
just qa-roomeq-quick
just qa-roomeq-multi-measurement
just qa-plugins
```

## Building ARM binaries

On a ARM Mac M1-M5, you need docker:

```
just cross-macos-arm64
just cross-linux-arm64
just cross-windows-arm64
```

All the files land in `./dist`.

## Building ARM binaries

On a Linux X86 machine, you need docker:
```
just cross-linux-x86
just cross-windows-86
```
All the files land in `./dist`.

## Building on Windows X86

In a powershell terminal:
```
.\build\build-windows.bat
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

Generate a msix







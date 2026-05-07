#Requires -Version 5.1
<#
.SYNOPSIS
    Build script for SotF Windows binaries.

.DESCRIPTION
    Builds two binaries with different CRT-linkage policies:

      sotf-tui.exe     -- ALWAYS static CRT (+crt-static). Pure-Rust binary,
                          no C++ deps, no MSVC DLL imports. Single
                          self-contained .exe -- no VCRedist / VCLibs needed
                          on the target machine.

      sotf-desktop.exe -- DYNAMIC CRT by default. Goes into the MSIX, which
                          declares Microsoft.VCLibs.140.00.UWPDesktop as a
                          framework dependency. The C++ side of gpui_windows
                          (Skia) makes a static-CRT GPUI build a less
                          natural fit. Pass -Static to force +crt-static
                          for direct (non-MSIX) GPUI distribution.

    Static and dynamic builds use separate target dirs (target\release\ vs
    target\static-crt\release\) so RUSTFLAGS changes don't cache-bust the
    other build.

.PARAMETER Clean
    Clean target dirs for the selected packages before building.

.PARAMETER TuiOnly
    Build only sotf-tui (skip the GPUI desktop binary).

.PARAMETER GpuiOnly
    Build only sotf-desktop (skip the TUI binary).

.PARAMETER Static
    Force static-CRT linkage for sotf-desktop too. The TUI is always static
    regardless of this flag.

.PARAMETER Help
    Show this help message.

.EXAMPLE
    .\build-windows.ps1
    sotf-tui static, sotf-desktop dynamic (MSIX-ready).

.EXAMPLE
    .\build-windows.ps1 -TuiOnly
    Just the static, self-contained sotf-tui.exe.

.EXAMPLE
    .\build-windows.ps1 -Static
    Both binaries static-CRT (for non-MSIX direct distribution).
#>

[CmdletBinding()]
param(
    [switch]$Clean,
    [switch]$TuiOnly,
    [switch]$GpuiOnly,
    [switch]$Static,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64" -or $env:PROCESSOR_IDENTIFIER -like "*ARM*") {
        "arm64"
    } else {
        "x64"
    }
} else {
    "x86"
}

# Per-binary configuration. StaticCrt drives the linkage policy:
#   $true  -> +crt-static, target\static-crt\release\
#   $false -> default CRT, target\release\
# sotf-tui is hard-wired static (single self-contained .exe). sotf-desktop
# follows the -Static flag (default off; MSIX picks up the dynamic build and
# pulls VCLibs via the framework dependency in AppxManifest.xml).
$Binaries = @(
    @{
        Name      = "sotf-desktop"
        Binary    = "sotf-desktop.exe"
        Package   = "sotf-gpui"
        Skip      = $TuiOnly
        StaticCrt = [bool]$Static
    },
    @{
        Name      = "sotf-tui"
        Binary    = "sotf-tui.exe"
        Package   = "sotf-tui"
        Skip      = $GpuiOnly
        StaticCrt = $true
    }
)

# Paths
$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Resolve-Path "$ScriptDir\..").Path

# Extract version from Cargo.toml
$CargoToml = Get-Content "$ProjectRoot\Cargo.toml" -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Write-Error "ERROR: Could not extract version from Cargo.toml"
    exit 1
}

$DynamicBuildDir = "$ProjectRoot\target\release"
$StaticBuildDir  = "$ProjectRoot\target\static-crt\release"
$DistDir         = "$ProjectRoot\dist"

# Colors for output
function Write-Info    { param($Message) Write-Host "[INFO] $Message"    -ForegroundColor Blue }
function Write-Success { param($Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Warn    { param($Message) Write-Host "[WARNING] $Message" -ForegroundColor Yellow }
function Write-Err     { param($Message) Write-Host "[ERROR] $Message"   -ForegroundColor Red }

function Show-Help {
    Get-Help $MyInvocation.PSCommandPath -Detailed
    exit 0
}

# Resolve the target/release dir for a given binary based on its linkage.
function Get-BinaryBuildDir {
    param($BinConfig)
    if ($BinConfig.StaticCrt) { return $StaticBuildDir } else { return $DynamicBuildDir }
}

function Test-Prerequisites {
    Write-Info "Checking prerequisites..."
    Write-Info "Detected architecture: $Arch"

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Err "Rust/Cargo is not installed"
        Write-Info "Install from: https://rustup.rs"
        exit 1
    }

    # Check Visual Studio Build Tools
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vsWhere) {
        $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (-not $vsPath) {
            Write-Warn "Visual Studio C++ Build Tools not found"
            Write-Info "Install from: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
        }
    }

    # Check LLVM for bliss-audio
    if (-not (Test-Path "C:\Program Files\LLVM\bin\libclang.dll")) {
        Write-Warn "LLVM not found - required for bliss-audio"
        Write-Info "Install with: winget install LLVM.LLVM"
    }

    Write-Success "Prerequisites check passed"
}

function Clear-Build {
    if (-not $Clean) { return }

    Write-Info "Cleaning build artifacts..."
    Push-Location $ProjectRoot
    try {
        foreach ($bin in $Binaries) {
            if ($bin.Skip) { continue }
            # `cargo clean -p` respects --target-dir; clean both possible
            # target dirs for the package so a previous mode's artifacts
            # don't linger.
            & cargo clean -p $bin.Package
            & cargo clean -p $bin.Package --target-dir "$ProjectRoot\target\static-crt"
        }
    }
    finally {
        Pop-Location
    }
}

function Build-Binary {
    param($BinConfig)

    if ($BinConfig.Skip) {
        Write-Info "Skipping $($BinConfig.Name)..."
        return $true
    }

    $linkage = if ($BinConfig.StaticCrt) { "static-CRT" } else { "dynamic-CRT" }
    Write-Info "Building $($BinConfig.Name) ($linkage release)..."

    # Restate CPU features in RUSTFLAGS when we override it: setting
    # $env:RUSTFLAGS here completely replaces the rustflags from
    # .cargo/config.toml's [target.<triple>] block, including the +sse... /
    # +neon entries. We only do this for the static-CRT path.
    $cpuFeatures = if ($Arch -eq "arm64") {
        "+neon"
    } else {
        "+sse,+sse2,+sse3,+ssse3,+sse4.1,+sse4.2,+avx,+avx2"
    }

    Push-Location $ProjectRoot
    try {
        $cargoArgs = @('build','--release','--package',$BinConfig.Package)

        if ($BinConfig.StaticCrt) {
            $env:RUSTFLAGS = "-C target-feature=+crt-static,$cpuFeatures"
            # Separate target dir so the static and dynamic builds don't
            # invalidate each other's incremental cache when both binaries
            # are built in one invocation.
            $cargoArgs += @('--target-dir', "$ProjectRoot\target\static-crt")
            Write-Info "  RUSTFLAGS: $($env:RUSTFLAGS)"
            Write-Info "  target-dir: target\static-crt"
        } else {
            # Don't set RUSTFLAGS at all -- let .cargo/config.toml's
            # [target.x86_64-pc-windows-msvc] / [target.aarch64-pc-windows-msvc]
            # rustflags apply.
            $env:RUSTFLAGS = $null
            Write-Info "  Using default CRT linkage from .cargo/config.toml"
        }

        & cargo @cargoArgs

        if ($LASTEXITCODE -ne 0) {
            Write-Err "Build failed for $($BinConfig.Name)"
            return $false
        }
    }
    finally {
        $env:RUSTFLAGS = $null
        Pop-Location
    }

    $buildDir   = Get-BinaryBuildDir $BinConfig
    $binaryPath = "$buildDir\$($BinConfig.Binary)"
    if (-not (Test-Path $binaryPath)) {
        Write-Err "Binary not found at $binaryPath"
        return $false
    }

    Write-Success "$($BinConfig.Name) built successfully ($linkage)"
    return $true
}

function New-Distribution {
    Write-Info "Creating distribution package..."

    # The dist suffix reflects the GPUI linkage, since the TUI is always
    # static -- the user-visible difference between archives is whether
    # SotF (desktop) needs VCLibs/VCRedist or not.
    $staticSuffix = if ($Static) { "-static" } else { "" }
    $distName     = "sotf-$Version-windows-$Arch$staticSuffix"
    $stagingDir   = "$DistDir\$distName"

    New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
    if (Test-Path $stagingDir) { Remove-Item -Recurse -Force $stagingDir }
    New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null

    # Copy binaries from their respective target dirs.
    foreach ($bin in $Binaries) {
        if ($bin.Skip) { continue }
        $srcPath = "$(Get-BinaryBuildDir $bin)\$($bin.Binary)"
        if (Test-Path $srcPath) {
            Copy-Item $srcPath -Destination $stagingDir
            $tag = if ($bin.StaticCrt) { "static" } else { "dynamic" }
            Write-Info "Added $($bin.Binary) ($tag)"
        }
    }

    # No runtime DLLs to ship -- pure-Rust binary, MSVC CRT comes from VCRedist
    # (dynamic build) or is statically linked (-Static build).

    # Copy assets excluding demo-audio (distributed separately as sotf-demo.zip)
    $assetsDir = "$ProjectRoot\crates\app-gpui\assets"
    if (Test-Path $assetsDir) {
        Copy-Item -Recurse $assetsDir -Destination "$stagingDir\assets"
        $demoAudioDir = "$stagingDir\assets\demo-audio"
        if (Test-Path $demoAudioDir) {
            Remove-Item -Recurse -Force $demoAudioDir
        }
    }

    # README. Per-binary requirements: TUI is always self-contained;
    # sotf-desktop needs VCRedist on the user's box unless built with -Static.
    $desktopReq = if ($Static) {
        "  sotf-desktop.exe : self-contained (static CRT)"
    } else {
        "  sotf-desktop.exe : needs Visual C++ Redistributable 2015-2022 (usually pre-installed on Windows 10/11)"
    }
    $readme = @"
SotF v$Version (Windows $Arch)
==============================

A high-quality audio player with advanced EQ and upmixing capabilities.

Included Binaries
-----------------
- sotf-desktop.exe : GPUI-based graphical player
- sotf-tui.exe     : Terminal UI player

Running
-------
GUI: Double-click sotf-desktop.exe
TUI: Run sotf-tui.exe from command line or PowerShell

Requirements
------------
- Windows 10/11 $Arch
  sotf-tui.exe     : self-contained (static CRT, no extra runtime needed)
$desktopReq

For more information, visit: https://github.com/pierreaubert/sotf
"@
    $readme | Out-File -FilePath "$stagingDir\README.txt" -Encoding UTF8

    $zipPath = "$DistDir\$distName.zip"
    if (Test-Path $zipPath) { Remove-Item $zipPath }

    Write-Info "Creating zip archive..."
    Compress-Archive -Path "$stagingDir\*" -DestinationPath $zipPath -CompressionLevel Optimal

    Remove-Item -Recurse -Force $stagingDir

    Write-Success "Distribution created: $zipPath"
    return $zipPath
}

function Main {
    if ($Help) { Show-Help; return }

    Write-Info "=========================================="
    Write-Info "Building SotF v$Version for Windows $Arch"
    Write-Info "  TUI         : static-CRT (always)"
    Write-Info "  Desktop/MSIX: $(if ($Static) { 'static-CRT (-Static)' } else { 'dynamic-CRT' })"
    Write-Info "=========================================="

    Test-Prerequisites
    Clear-Build

    $allSuccess = $true
    foreach ($bin in $Binaries) {
        if (-not (Build-Binary $bin)) { $allSuccess = $false }
    }

    if (-not $allSuccess) {
        Write-Err "Some builds failed"
        exit 1
    }

    $zipPath = New-Distribution

    Write-Info "=========================================="
    Write-Success "Build complete!"
    Write-Info "=========================================="

    if (Test-Path $zipPath) {
        $size = (Get-Item $zipPath).Length / 1MB
        Write-Info "Package: $zipPath"
        Write-Info ("Size: {0:N2} MB" -f $size)
    }
}

Main

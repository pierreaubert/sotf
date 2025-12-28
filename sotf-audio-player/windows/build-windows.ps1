#Requires -Version 5.1
<#
.SYNOPSIS
    Build script for SotF Player Windows applications

.DESCRIPTION
    Creates distributable packages with static binaries for Windows

.PARAMETER InstallDeps
    Install dependencies using vcpkg

.PARAMETER Clean
    Clean build directory before building

.PARAMETER TuiOnly
    Build only sotf-tui (skip GPUI)

.PARAMETER GpuiOnly
    Build only SotF GPUI (skip TUI)

.PARAMETER Static
    Build static binaries with static CRT and static libraries

.PARAMETER Help
    Show this help message

.EXAMPLE
    .\build-windows.ps1
    Build both release binaries (SotF and sotf-tui)

.EXAMPLE
    .\build-windows.ps1 -InstallDeps
    Install dependencies and build

.EXAMPLE
    .\build-windows.ps1 -TuiOnly
    Build only the TUI version

.EXAMPLE
    .\build-windows.ps1 -Static
    Build static binaries (static CRT + static libraries)
#>

[CmdletBinding()]
param(
    [switch]$InstallDeps,
    [switch]$Clean,
    [switch]$TuiOnly,
    [switch]$GpuiOnly,
    [switch]$Static,
    [switch]$Help
)

# Stop on first error
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

# Configuration
$Binaries = @(
    @{
        Name = "SotF"
        Binary = "SotF.exe"
        Package = "sotf-gpui"
        Skip = $TuiOnly
    },
    @{
        Name = "sotf-tui"
        Binary = "sotf-tui.exe"
        Package = "sotf-tui"
        Skip = $GpuiOnly
    }
)

# Paths
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Resolve-Path "$ScriptDir\..\..").Path

# Extract version from Cargo.toml
$CargoToml = Get-Content "$ProjectRoot\Cargo.toml" -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Write-Error "ERROR: Could not extract version from Cargo.toml"
    exit 1
}

$BuildDir = "$ProjectRoot\target\release"
$DistDir = "$ProjectRoot\dist"

# vcpkg configuration - use C:\vcpkg symlink or VCPKG_ROOT
$VcpkgRoot = if (Test-Path "C:\vcpkg") {
    "C:\vcpkg"
} elseif ($env:VCPKG_ROOT) {
    $env:VCPKG_ROOT
} else {
    "$env:USERPROFILE\vcpkg"
}

# Select triplet based on static/dynamic build
if ($Static) {
    $VcpkgTriplet = "$Arch-windows-static"
    $BuildType = "static"
} else {
    $VcpkgTriplet = "$Arch-windows"
    $BuildType = "dynamic"
}

# Required vcpkg packages
$VcpkgPackages = @(
    "openblas:$VcpkgTriplet",
    "nlopt:$VcpkgTriplet"
)

# Colors for output
function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Success { param($Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Warn { param($Message) Write-Host "[WARNING] $Message" -ForegroundColor Yellow }
function Write-Err { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red }

function Show-Help {
    Get-Help $MyInvocation.PSCommandPath -Detailed
    exit 0
}

function Test-Prerequisites {
    Write-Info "Checking prerequisites..."
    Write-Info "Detected architecture: $Arch"

    # Check Rust/Cargo
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Err "Rust/Cargo is not installed"
        Write-Info "Install from: https://rustup.rs"
        exit 1
    }

    # Check vcpkg
    if (-not (Test-Path "$VcpkgRoot\vcpkg.exe")) {
        Write-Err "vcpkg not found at $VcpkgRoot"
        Write-Info "Either:"
        Write-Info "  1. Create symlink: New-Item -ItemType SymbolicLink -Path 'C:\vcpkg' -Target '<your-vcpkg-path>'"
        Write-Info "  2. Or set VCPKG_ROOT environment variable"
        exit 1
    }

    Write-Info "Using vcpkg at: $VcpkgRoot"

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

function Install-Dependencies {
    if (-not $InstallDeps) {
        return
    }

    Write-Info "Installing dependencies with vcpkg..."

    Push-Location $VcpkgRoot
    try {
        foreach ($package in $VcpkgPackages) {
            Write-Info "Installing $package..."
            & .\vcpkg.exe install $package
            if ($LASTEXITCODE -ne 0) {
                Write-Err "Failed to install $package"
                exit 1
            }
        }

        # Integrate vcpkg with cargo
        Write-Info "Integrating vcpkg..."
        & .\vcpkg.exe integrate install
    }
    finally {
        Pop-Location
    }

    Write-Success "Dependencies installed"
}

function Clear-Build {
    if (-not $Clean) {
        return
    }

    Write-Info "Cleaning build directory..."
    Push-Location $ProjectRoot
    try {
        foreach ($bin in $Binaries) {
            if (-not $bin.Skip) {
                & cargo clean -p $bin.Package
            }
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

    Write-Info "Building $($BinConfig.Name) ($BuildType release)..."

    Push-Location $ProjectRoot
    try {
        # Set environment for vcpkg integration
        $env:VCPKG_ROOT = $VcpkgRoot
        $env:VCPKGRS_TRIPLET = $VcpkgTriplet

        if ($Static) {
            # For static builds, use static CRT and link against static libraries
            # The static triplet provides static .lib files
            # Include architecture-specific CPU features for optimal performance
            $cpuFeatures = if ($Arch -eq "arm64") {
                "+neon"
            } else {
                "+sse,+sse2,+sse3,+ssse3,+sse4.1,+sse4.2,+avx,+avx2"
            }
            $env:RUSTFLAGS = "-C target-feature=+crt-static,$cpuFeatures -C link-arg=/LIBPATH:$VcpkgRoot\installed\$VcpkgTriplet\lib -C link-arg=openblas.lib -C link-arg=nlopt.lib"
            Write-Info "Using static linkage with RUSTFLAGS: $($env:RUSTFLAGS)"
        }

        & cargo build --release --package $BinConfig.Package

        if ($LASTEXITCODE -ne 0) {
            Write-Err "Build failed for $($BinConfig.Name)"
            return $false
        }
    }
    finally {
        # Clear RUSTFLAGS to avoid affecting other builds
        $env:RUSTFLAGS = $null
        Pop-Location
    }

    $binaryPath = "$BuildDir\$($BinConfig.Binary)"
    if (-not (Test-Path $binaryPath)) {
        Write-Err "Binary not found at $binaryPath"
        return $false
    }

    Write-Success "$($BinConfig.Name) built successfully"
    return $true
}

function New-Distribution {
    Write-Info "Creating distribution package..."

    $staticSuffix = if ($Static) { "-static" } else { "" }
    $distName = "sotf-$Version-windows-$Arch$staticSuffix"
    $stagingDir = "$DistDir\$distName"

    # Create directories
    New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
    if (Test-Path $stagingDir) {
        Remove-Item -Recurse -Force $stagingDir
    }
    New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null

    # Copy binaries
    foreach ($bin in $Binaries) {
        if (-not $bin.Skip) {
            $srcPath = "$BuildDir\$($bin.Binary)"
            if (Test-Path $srcPath) {
                Copy-Item $srcPath -Destination $stagingDir
                Write-Info "Added $($bin.Binary)"
            }
        }
    }

    # Copy assets if they exist
    $assetsDir = "$ScriptDir\..\assets"
    if (Test-Path $assetsDir) {
        Copy-Item -Recurse $assetsDir -Destination "$stagingDir\assets"
    }

    # Create README
    $buildTypeDesc = if ($Static) { "Static Build (no external dependencies)" } else { "Dynamic Build" }
    $reqsDesc = if ($Static) {
        "- Windows 10/11 $Arch"
    } else {
        "- Windows 10/11 $Arch`n- Visual C++ Redistributable 2019 or later (usually pre-installed)"
    }
    $readme = @"
SotF Player v$Version ($buildTypeDesc)
======================

A high-quality audio player with advanced EQ and upmixing capabilities.

Included Binaries
-----------------
- SotF.exe      : GPUI-based graphical player
- sotf-tui.exe  : Terminal UI player

Running
-------
GUI: Double-click SotF.exe
TUI: Run sotf-tui.exe from command line or PowerShell

Requirements
------------
$reqsDesc

For more information, visit: https://github.com/pierreaubert/sotf
"@
    $readme | Out-File -FilePath "$stagingDir\README.txt" -Encoding UTF8

    # Create zip file
    $zipPath = "$DistDir\$distName.zip"
    if (Test-Path $zipPath) {
        Remove-Item $zipPath
    }

    Write-Info "Creating zip archive..."
    Compress-Archive -Path "$stagingDir\*" -DestinationPath $zipPath -CompressionLevel Optimal

    # Clean up staging directory
    Remove-Item -Recurse -Force $stagingDir

    Write-Success "Distribution created: $zipPath"
    return $zipPath
}

function Main {
    if ($Help) {
        Show-Help
        return
    }

    Write-Info "=========================================="
    Write-Info "Building SotF v$Version for Windows $Arch ($BuildType)"
    Write-Info "=========================================="

    Test-Prerequisites
    Install-Dependencies
    Clear-Build

    $allSuccess = $true
    foreach ($bin in $Binaries) {
        if (-not (Build-Binary $bin)) {
            $allSuccess = $false
        }
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

# Run main
Main

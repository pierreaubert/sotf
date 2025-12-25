#Requires -Version 5.1
<#
.SYNOPSIS
    Build script for SotF Player Windows application

.DESCRIPTION
    Creates a distributable package with the GPUI binary for Windows 11

.PARAMETER InstallDeps
    Install dependencies using vcpkg

.PARAMETER Clean
    Clean build directory before building

.PARAMETER Help
    Show this help message

.EXAMPLE
    .\build-windows.ps1
    Build release binary

.EXAMPLE
    .\build-windows.ps1 -InstallDeps
    Install dependencies and build

.EXAMPLE
    .\build-windows.ps1 -Clean
    Clean and rebuild
#>

[CmdletBinding()]
param(
    [switch]$InstallDeps,
    [switch]$Clean,
    [switch]$Help
)

# Stop on first error
$ErrorActionPreference = "Stop"

# Configuration
$AppName = "SotF"
$BinaryName = "SotF.exe"
$PackageName = "sotf-gpui"

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

# vcpkg configuration
$VcpkgRoot = if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } else { "$env:USERPROFILE\vcpkg" }
$VcpkgTriplet = "x64-windows-static"

# Required vcpkg packages for GPUI
$VcpkgPackages = @(
    "openssl:$VcpkgTriplet"
)

# Colors for output
function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Success { param($Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Warning { param($Message) Write-Host "[WARNING] $Message" -ForegroundColor Yellow }
function Write-Error { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red }

function Show-Help {
    Get-Help $MyInvocation.PSCommandPath -Detailed
    exit 0
}

function Test-Prerequisites {
    Write-Info "Checking prerequisites..."

    # Check Rust/Cargo
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Error "Rust/Cargo is not installed"
        Write-Info "Install from: https://rustup.rs"
        exit 1
    }

    # Check vcpkg if installing deps
    if ($InstallDeps) {
        if (-not (Test-Path "$VcpkgRoot\vcpkg.exe")) {
            Write-Error "vcpkg not found at $VcpkgRoot"
            Write-Info "Install vcpkg:"
            Write-Info "  git clone https://github.com/Microsoft/vcpkg.git $VcpkgRoot"
            Write-Info "  cd $VcpkgRoot && .\bootstrap-vcpkg.bat"
            Write-Info "Or set VCPKG_ROOT environment variable to your vcpkg installation"
            exit 1
        }
    }

    # Check Visual Studio Build Tools
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vsWhere) {
        $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (-not $vsPath) {
            Write-Warning "Visual Studio C++ Build Tools not found"
            Write-Info "Install from: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
        }
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
                Write-Error "Failed to install $package"
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
        & cargo clean -p $PackageName
    }
    finally {
        Pop-Location
    }
}

function Build-Binary {
    Write-Info "Building release binary..."

    Push-Location $ProjectRoot
    try {
        # Set environment for vcpkg integration
        $env:VCPKG_ROOT = $VcpkgRoot
        $env:VCPKGRS_TRIPLET = $VcpkgTriplet

        # Build with static CRT for better portability
        $env:RUSTFLAGS = "-C target-feature=+crt-static"

        Write-Info "Building $PackageName..."
        & cargo build --release --package $PackageName

        if ($LASTEXITCODE -ne 0) {
            Write-Error "Build failed"
            exit 1
        }
    }
    finally {
        Pop-Location
    }

    $binaryPath = "$BuildDir\$BinaryName"
    if (-not (Test-Path $binaryPath)) {
        Write-Error "Binary not found at $binaryPath"
        exit 1
    }

    Write-Success "Binary built successfully"
}

function New-Distribution {
    Write-Info "Creating distribution package..."

    $arch = "x64"
    $distName = "$AppName-$Version-windows-$arch"
    $stagingDir = "$DistDir\$distName"

    # Create directories
    New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
    if (Test-Path $stagingDir) {
        Remove-Item -Recurse -Force $stagingDir
    }
    New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null

    # Copy binary
    Copy-Item "$BuildDir\$BinaryName" -Destination $stagingDir

    # Copy assets if they exist
    $assetsDir = "$ScriptDir\..\assets"
    if (Test-Path $assetsDir) {
        Copy-Item -Recurse $assetsDir -Destination "$stagingDir\assets"
    }

    # Create README
    $readme = @"
SotF Player v$Version
======================

A high-quality audio player with advanced EQ and upmixing capabilities.

Running
-------
Double-click SotF.exe or run from command line:
  .\SotF.exe

Requirements
------------
- Windows 10/11 x64
- Visual C++ Redistributable 2019 or later (usually pre-installed)

For more information, visit: https://github.com/coderdelphit/stypes
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
}

function New-Installer {
    # Optional: Create installer using Inno Setup or WiX
    # This is a placeholder for future implementation

    $innoSetup = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    if (-not (Test-Path $innoSetup)) {
        Write-Info "Inno Setup not found, skipping installer creation"
        Write-Info "Install from: https://jrsoftware.org/isinfo.php"
        return
    }

    # TODO: Create .iss script and build installer
    Write-Info "Installer creation not yet implemented"
}

function Main {
    if ($Help) {
        Show-Help
        return
    }

    Write-Info "=========================================="
    Write-Info "Building $AppName v$Version for Windows"
    Write-Info "=========================================="

    Test-Prerequisites
    Install-Dependencies
    Clear-Build
    Build-Binary
    New-Distribution

    Write-Info "=========================================="
    Write-Success "Build complete!"
    Write-Info "=========================================="

    $arch = "x64"
    $distName = "$AppName-$Version-windows-$arch"
    $zipPath = "$DistDir\$distName.zip"

    if (Test-Path $zipPath) {
        $size = (Get-Item $zipPath).Length / 1MB
        Write-Info "Package: $zipPath"
        Write-Info ("Size: {0:N2} MB" -f $size)
    }
}

# Run main
Main

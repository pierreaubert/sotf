#Requires -Version 5.1
<#
.SYNOPSIS
    Build MSIX package for SotF Player

.DESCRIPTION
    Creates an MSIX package from pre-built Windows binaries.
    Run build-windows.ps1 first to produce the binaries.

.PARAMETER CertPath
    Path to the .pfx signing certificate. If not provided, creates a
    self-signed certificate for local testing.

.PARAMETER CertPassword
    Password for the .pfx certificate (if password-protected).

.PARAMETER SkipSign
    Skip code signing (produces unsigned MSIX for testing only).

.PARAMETER Help
    Show this help message.

.EXAMPLE
    .\build-msix.ps1
    Build MSIX with self-signed certificate for local testing

.EXAMPLE
    .\build-msix.ps1 -CertPath .\sotf.pfx -CertPassword "secret"
    Build MSIX signed with provided certificate

.EXAMPLE
    .\build-msix.ps1 -SkipSign
    Build unsigned MSIX package
#>

[CmdletBinding()]
param(
    [string]$CertPath,
    [string]$CertPassword,
    [switch]$SkipSign,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Resolve-Path "$ScriptDir\..").Path

# Extract version from Cargo.toml
$CargoToml = Get-Content "$ProjectRoot\Cargo.toml" -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Write-Error "Could not extract version from Cargo.toml"
    exit 1
}
# MSIX requires 4-part version
$MsixVersion = "$Version.0"

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

$BuildDir = "$ProjectRoot\target\release"
$MsixStaging = "$ProjectRoot\dist\msix-staging"
$MsixOutput = "$ProjectRoot\dist\SotF-$Version-windows-$Arch.msix"

$Publisher = "CN=Pierre Aubert, O=Spinorama, C=FR"

function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Success { param($Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Err { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red }

function Show-Help {
    Get-Help $MyInvocation.PSCommandPath -Detailed
    exit 0
}

function Test-Prerequisites {
    Write-Info "Checking prerequisites..."

    # Check makeappx
    $makeappx = Get-Command makeappx.exe -ErrorAction SilentlyContinue
    if (-not $makeappx) {
        # Try Windows SDK paths
        $sdkPaths = @(
            "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\$Arch\makeappx.exe"
            "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe"
        )
        foreach ($pattern in $sdkPaths) {
            $found = Get-Item $pattern -ErrorAction SilentlyContinue | Sort-Object -Descending | Select-Object -First 1
            if ($found) {
                $script:MakeAppx = $found.FullName
                $script:SignTool = Join-Path (Split-Path $found.FullName) "signtool.exe"
                break
            }
        }
        if (-not $script:MakeAppx) {
            Write-Err "makeappx.exe not found. Install Windows 10 SDK."
            Write-Info "Download: https://developer.microsoft.com/windows/downloads/windows-sdk/"
            exit 1
        }
    } else {
        $script:MakeAppx = $makeappx.Source
        $script:SignTool = Join-Path (Split-Path $makeappx.Source) "signtool.exe"
    }
    Write-Info "Using makeappx: $($script:MakeAppx)"

    # Check binaries exist
    foreach ($bin in @("SotF.exe", "sotf-tui.exe")) {
        if (-not (Test-Path "$BuildDir\$bin")) {
            Write-Err "$bin not found in $BuildDir"
            Write-Info "Run build-windows.ps1 first to build the binaries."
            exit 1
        }
    }

    Write-Success "Prerequisites OK"
}

function New-StagingDirectory {
    Write-Info "Preparing MSIX staging directory..."

    if (Test-Path $MsixStaging) {
        Remove-Item -Recurse -Force $MsixStaging
    }
    New-Item -ItemType Directory -Force -Path $MsixStaging | Out-Null
    New-Item -ItemType Directory -Force -Path "$MsixStaging\assets" | Out-Null

    # Copy binaries
    Copy-Item "$BuildDir\SotF.exe" -Destination $MsixStaging
    Copy-Item "$BuildDir\sotf-tui.exe" -Destination $MsixStaging

    # Copy nlopt.dll if present (dynamic builds)
    if (Test-Path "$BuildDir\nlopt.dll") {
        Copy-Item "$BuildDir\nlopt.dll" -Destination $MsixStaging
    }

    # Copy app assets (fonts, icons, targets - but not demo-audio)
    $assetsSource = "$ProjectRoot\crates\app-gpui\assets"
    if (Test-Path $assetsSource) {
        # Copy selectively - fonts, icons, headphone-targets
        foreach ($subdir in @("fonts", "icons", "headphone-targets")) {
            $src = "$assetsSource\$subdir"
            if (Test-Path $src) {
                Copy-Item -Recurse $src -Destination "$MsixStaging\assets\$subdir"
            }
        }
    }

    # Generate MSIX icon assets from source PNG
    # MSIX requires specific sizes: 44x44, 150x150, 310x150
    $sourcePng = "$assetsSource\sotf.png"
    if (Test-Path $sourcePng) {
        Copy-Item $sourcePng -Destination "$MsixStaging\assets\sotf-44x44.png"
        Copy-Item $sourcePng -Destination "$MsixStaging\assets\sotf-150x150.png"
        Copy-Item $sourcePng -Destination "$MsixStaging\assets\sotf-310x150.png"
        Write-Info "Copied icon assets (resize to exact dimensions for Store submission)"
    } else {
        Write-Err "Source icon not found at $sourcePng"
        Write-Info "MSIX requires icon assets. Add sotf.png to crates/app-gpui/assets/"
        exit 1
    }

    # Generate AppxManifest.xml with correct version and architecture
    $manifestTemplate = Get-Content "$ScriptDir\AppxManifest.xml" -Raw
    $manifest = $manifestTemplate -replace 'Version="[^"]*"', "Version=`"$MsixVersion`""
    $manifest = $manifest -replace 'ProcessorArchitecture="[^"]*"', "ProcessorArchitecture=`"$Arch`""
    $manifest | Out-File -FilePath "$MsixStaging\AppxManifest.xml" -Encoding UTF8

    Write-Success "Staging directory ready: $MsixStaging"
}

function New-SelfSignedCert {
    Write-Info "Creating self-signed certificate for testing..."

    $cert = New-SelfSignedCertificate `
        -Type Custom `
        -Subject $Publisher `
        -KeyUsage DigitalSignature `
        -FriendlyName "SotF MSIX Signing (Test)" `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")

    $script:CertThumbprint = $cert.Thumbprint
    Write-Info "Certificate thumbprint: $($cert.Thumbprint)"
    Write-Info "To trust this certificate on other machines, export and install it."
    Write-Success "Self-signed certificate created"

    return $cert.Thumbprint
}

function Build-Msix {
    Write-Info "Building MSIX package..."

    $distDir = Split-Path $MsixOutput
    New-Item -ItemType Directory -Force -Path $distDir | Out-Null

    if (Test-Path $MsixOutput) {
        Remove-Item $MsixOutput
    }

    & $script:MakeAppx pack /d $MsixStaging /p $MsixOutput /o
    if ($LASTEXITCODE -ne 0) {
        Write-Err "makeappx pack failed"
        exit 1
    }

    Write-Success "MSIX package created: $MsixOutput"
}

function Sign-Msix {
    if ($SkipSign) {
        Write-Info "Skipping code signing (unsigned package)"
        return
    }

    Write-Info "Signing MSIX package..."

    if ($CertPath) {
        # Sign with provided certificate
        $signArgs = @("sign", "/fd", "SHA256", "/f", $CertPath)
        if ($CertPassword) {
            $signArgs += @("/p", $CertPassword)
        }
        $signArgs += @("/t", "http://timestamp.digicert.com", $MsixOutput)
    } else {
        # Sign with self-signed certificate
        $thumbprint = New-SelfSignedCert
        $signArgs = @("sign", "/fd", "SHA256", "/sha1", $thumbprint, "/t", "http://timestamp.digicert.com", $MsixOutput)
    }

    & $script:SignTool @signArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Err "signtool failed"
        exit 1
    }

    Write-Success "MSIX package signed"
}

function Main {
    if ($Help) {
        Show-Help
        return
    }

    Write-Info "=========================================="
    Write-Info "Building SotF MSIX v$Version ($Arch)"
    Write-Info "=========================================="

    Test-Prerequisites
    New-StagingDirectory
    Build-Msix
    Sign-Msix

    # Cleanup staging
    Remove-Item -Recurse -Force $MsixStaging

    Write-Info "=========================================="
    Write-Success "MSIX build complete!"
    Write-Info "=========================================="

    if (Test-Path $MsixOutput) {
        $size = (Get-Item $MsixOutput).Length / 1MB
        Write-Info "Package: $MsixOutput"
        Write-Info ("Size: {0:N2} MB" -f $size)
        Write-Info ""
        Write-Info "To install locally (requires trusted certificate):"
        Write-Info "  Add-AppxPackage -Path $MsixOutput"
        Write-Info ""
        Write-Info "For Store submission, use a trusted code signing certificate."
    }
}

Main

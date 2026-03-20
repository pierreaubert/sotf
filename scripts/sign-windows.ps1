#Requires -Version 5.1
<#
.SYNOPSIS
    Sign Windows artifacts in dist/ using signtool or cosign

.DESCRIPTION
    Signs .exe and .zip files for distribution.
    Supports both Authenticode (signtool) and cosign (Sigstore) signing.

.PARAMETER CertificateThumbprint
    SHA1 thumbprint of the code signing certificate in the Windows certificate store

.PARAMETER PfxPath
    Path to a PFX/PKCS12 certificate file (alternative to thumbprint)

.PARAMETER PfxPassword
    Password for the PFX file

.PARAMETER TimestampServer
    Timestamp server URL (default: http://timestamp.digicert.com)

.PARAMETER UseCosign
    Use cosign (Sigstore) instead of Authenticode

.PARAMETER CosignKey
    Path to cosign private key (optional, uses keyless OIDC if omitted)

.PARAMETER Files
    Specific files to sign (default: all Windows artifacts in dist/)

.EXAMPLE
    .\sign-windows.ps1 -CertificateThumbprint "ABC123..."
    Sign all Windows artifacts in dist/ using a certificate from the store

.EXAMPLE
    .\sign-windows.ps1 -PfxPath "cert.pfx" -PfxPassword "secret"
    Sign using a PFX certificate file

.EXAMPLE
    .\sign-windows.ps1 -UseCosign
    Sign using cosign keyless (Sigstore OIDC)

.EXAMPLE
    .\sign-windows.ps1 -UseCosign -CosignKey "cosign.key"
    Sign using cosign with a local key
#>

[CmdletBinding()]
param(
    [string]$CertificateThumbprint,
    [string]$PfxPath,
    [string]$PfxPassword,
    [string]$TimestampServer = "http://timestamp.digicert.com",
    [switch]$UseCosign,
    [string]$CosignKey,
    [string[]]$Files,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Resolve-Path "$ScriptDir\..").Path
$DistDir = "$ProjectRoot\dist"

function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Success { param($Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Warn { param($Message) Write-Host "[WARNING] $Message" -ForegroundColor Yellow }
function Write-Err { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red }

function Show-Help {
    Get-Help $MyInvocation.PSCommandPath -Detailed
    exit 0
}

function Test-Prerequisites {
    if ($UseCosign) {
        if (-not (Get-Command cosign -ErrorAction SilentlyContinue)) {
            Write-Err "cosign is not installed"
            Write-Info "Install with: winget install sigstore.cosign"
            Write-Info "Or: go install github.com/sigstore/cosign/v2/cmd/cosign@latest"
            exit 1
        }
        if ($CosignKey -and -not (Test-Path $CosignKey)) {
            Write-Err "Cosign key not found: $CosignKey"
            exit 1
        }
    } else {
        # Look for signtool
        $signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
        if (-not $signtool) {
            # Try Windows SDK paths
            $sdkPaths = @(
                "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe",
                "${env:ProgramFiles(x86)}\Windows Kits\10\App Certification Kit\signtool.exe"
            )
            foreach ($pattern in $sdkPaths) {
                $found = Get-Item $pattern -ErrorAction SilentlyContinue | Sort-Object -Descending | Select-Object -First 1
                if ($found) {
                    $env:PATH = "$($found.DirectoryName);$env:PATH"
                    break
                }
            }

            if (-not (Get-Command signtool.exe -ErrorAction SilentlyContinue)) {
                Write-Err "signtool.exe not found"
                Write-Info "Install Windows SDK or use -UseCosign for Sigstore signing"
                exit 1
            }
        }

        if (-not $CertificateThumbprint -and -not $PfxPath) {
            Write-Err "Provide either -CertificateThumbprint or -PfxPath"
            Write-Info ""
            Write-Info "Examples:"
            Write-Info "  .\sign-windows.ps1 -CertificateThumbprint 'ABC123...'"
            Write-Info "  .\sign-windows.ps1 -PfxPath 'cert.pfx' -PfxPassword 'secret'"
            Write-Info "  .\sign-windows.ps1 -UseCosign  # Sigstore keyless"
            exit 1
        }
    }
}

function Sign-WithAuthenticode {
    param([string]$FilePath)

    $fileName = Split-Path -Leaf $FilePath
    Write-Info "Signing (Authenticode): $fileName"

    $signArgs = @(
        "sign",
        "/fd", "SHA256",
        "/td", "SHA256",
        "/tr", $TimestampServer
    )

    if ($CertificateThumbprint) {
        $signArgs += @("/sha1", $CertificateThumbprint)
    } elseif ($PfxPath) {
        $signArgs += @("/f", $PfxPath)
        if ($PfxPassword) {
            $signArgs += @("/p", $PfxPassword)
        }
    }

    $signArgs += $FilePath

    & signtool.exe @signArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Failed to sign: $fileName"
        exit 1
    }

    # Verify
    & signtool.exe verify /pa /v $FilePath | Out-Null
    Write-Success "Signed: $fileName"
}

function Sign-WithCosign {
    param([string]$FilePath)

    $fileName = Split-Path -Leaf $FilePath
    $sigPath = "$FilePath.sig"
    $certPath = "$FilePath.cert"

    Write-Info "Signing (cosign): $fileName"

    if ($CosignKey) {
        & cosign sign-blob --key $CosignKey --output-signature $sigPath $FilePath
        Write-Success "Signed (key): $fileName"
        Write-Info "  Signature: $(Split-Path -Leaf $sigPath)"
    } else {
        & cosign sign-blob --output-signature $sigPath --output-certificate $certPath $FilePath
        Write-Success "Signed (keyless): $fileName"
        Write-Info "  Signature:   $(Split-Path -Leaf $sigPath)"
        Write-Info "  Certificate: $(Split-Path -Leaf $certPath)"
    }
}

function Sign-File {
    param([string]$FilePath)

    if (-not (Test-Path $FilePath)) {
        Write-Warn "File not found: $FilePath"
        return
    }

    if ($UseCosign) {
        Sign-WithCosign $FilePath
    } else {
        $ext = [System.IO.Path]::GetExtension($FilePath).ToLower()
        switch ($ext) {
            ".exe"  { Sign-WithAuthenticode $FilePath }
            ".msi"  { Sign-WithAuthenticode $FilePath }
            ".msix" { Sign-WithAuthenticode $FilePath }
            ".appx" { Sign-WithAuthenticode $FilePath }
            ".zip" {
                Write-Warn "Authenticode cannot sign .zip files, use -UseCosign: $(Split-Path -Leaf $FilePath)"
            }
            default {
                Write-Warn "Unsupported file type for Authenticode: $ext (use -UseCosign)"
            }
        }
    }
}

function Test-IsWindowsArtifact {
    param([string]$FilePath)
    $name = Split-Path -Leaf $FilePath
    return ($name -like "*windows*" -or $name -like "*.exe" -or $name -like "*win*")
}

function Main {
    if ($Help) {
        Show-Help
        return
    }

    Write-Info "=========================================="
    Write-Info "Windows Artifact Signing"
    Write-Info "=========================================="

    if ($UseCosign) {
        if ($CosignKey) {
            Write-Info "Mode: cosign (key-based)"
        } else {
            Write-Info "Mode: cosign (keyless OIDC)"
        }
    } else {
        Write-Info "Mode: Authenticode (signtool)"
    }

    Test-Prerequisites

    if ($Files) {
        foreach ($f in $Files) {
            Sign-File $f
        }
    } else {
        $found = $false
        Get-ChildItem -Path $DistDir -File | Where-Object {
            $_.Extension -notin @(".sig", ".cert") -and
            (Test-IsWindowsArtifact $_.FullName)
        } | ForEach-Object {
            Sign-File $_.FullName
            $found = $true
        }

        if (-not $found) {
            Write-Warn "No Windows artifacts found in $DistDir"
            Write-Info "Run 'just cross-windows-arm64' or 'just cross-windows-x86' first"
            exit 1
        }
    }

    Write-Info "=========================================="
    Write-Success "Signing complete!"
    Write-Info "=========================================="
}

Main

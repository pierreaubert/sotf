#Requires -Version 5.1
<#
.SYNOPSIS
    Build the SotF MSIX package on Windows (native PowerShell).

.DESCRIPTION
    Native Windows port of scripts/build-msix.sh. Runs as a real Windows
    process so we don't need Git Bash / cygpath / MSYS_NO_PATHCONV / a
    third-party `zip.exe` on PATH -- MakeAppx.exe and SignTool.exe live in the
    Windows SDK and are invoked directly with their normal `/o` / `/v` /
    `/d` / `/p` flags.

    Pipeline:
      1. Locate MakeAppx.exe (Windows SDK).
      2. Stage <staging> with the Windows binaries, app assets, icons, and a
         rendered AppxManifest.xml (Version + ProcessorArchitecture injected).
      3. Validate the manifest (well-formed XML; xs:sequence ordering of
         <Capabilities>; Identity Version is 4-part; runFullTrust is declared
         when an Application uses Windows.FullTrustApplication).
      4. (-Sign) Sign the staged .exe files with SignTool.exe.
      5. Pack via `MakeAppx.exe pack /o /v /d <staging> /p <output>`.
      6. (-Sign) Sign the resulting .msix.

.PARAMETER Arch
    Target arch for the dist filename: x86_64 (default) or arm64. The
    AppxManifest's ProcessorArchitecture is set to the Windows-native synonym
    (x64 for x86_64; arm64 for arm64). x64 / aarch64 are accepted as aliases.

.PARAMETER BuildDir
    Directory containing the pre-built sotf-desktop.exe / sotf-tui.exe. If
    not given, common cargo target dirs are searched.

.PARAMETER Sign
    Authenticode-sign the exe files and the resulting .msix. Requires either
    $env:WINDOWS_CERT_THUMBPRINT (cert in CurrentUser\My) OR
    $env:WINDOWS_CERT_FILE (+ optional WINDOWS_CERT_PASSWORD).

.EXAMPLE
    .\build-msix.ps1 -Arch x86_64
    .\build-msix.ps1 -Arch x86_64 -Sign
#>
[CmdletBinding()]
param(
    [ValidateSet('x86_64','x64','arm64','aarch64')]
    [string]$Arch = 'x86_64',
    [string]$BuildDir,
    [switch]$Sign,

    # Code-signing inputs. CLI params take precedence over the same-named
    # WINDOWS_CERT_* environment variables. Params exist so a remote driver
    # (e.g. scripts/build-release-local.sh on macOS, SSH'ing to Windows) can
    # pass credentials through cleanly -- $env:WINDOWS_CERT_* doesn't survive
    # an SSH hop unless the sshd is configured with AcceptEnv, which it
    # usually isn't on Windows OpenSSH.
    [string]$CertThumbprint,
    [string]$CertFile,
    [string]$CertPassword,
    [string]$TimestampUrl,

    # Turn on PowerShell's built-in line-level tracer (Set-PSDebug -Trace 1).
    # Useful when you want to see exactly which line the script is on when
    # something fails. Use -TraceLevel 2 for very chatty (every assignment).
    [switch]$Trace,

    [ValidateRange(0,2)]
    [int]$TraceLevel = 1,

    # Path to a transcript log. Defaults to
    # dist/build-msix-<version>-<arch>.log. Use $null to disable.
    [string]$LogFile
)

$ErrorActionPreference = 'Stop'

# Honour -Verbose / $VerbosePreference. Without this, Write-Verbose calls
# inside the script's helper functions stay silent under the SSH-launched
# process unless the caller explicitly forwards $VerbosePreference.
if ($PSBoundParameters.ContainsKey('Verbose')) {
    $VerbosePreference = if ($PSBoundParameters['Verbose']) { 'Continue' } else { 'SilentlyContinue' }
}

# ---------------------------------------------------------------------------
# Paths and version
# ---------------------------------------------------------------------------
$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Resolve-Path (Join-Path $ScriptDir '..')).Path
$DistDir     = Join-Path $ProjectRoot 'dist'

$cargoToml = Join-Path $ProjectRoot 'Cargo.toml'
$Version = (Get-Content $cargoToml | Where-Object { $_ -match '^version = "([^"]+)"' } |
            Select-Object -First 1 |
            ForEach-Object { $Matches[1] })
if (-not $Version) {
    throw "Could not extract version from $cargoToml"
}
$MsixVersion = "$Version.0"   # MSIX requires Major.Minor.Build.Revision

# Map -Arch (filename convention) -> (DistArch, MsixArch)
switch ($Arch) {
    'x86_64'  { $DistArch = 'x86_64'; $MsixArch = 'x64' }
    'x64'     { $DistArch = 'x86_64'; $MsixArch = 'x64' }
    'arm64'   { $DistArch = 'arm64';  $MsixArch = 'arm64' }
    'aarch64' { $DistArch = 'arm64';  $MsixArch = 'arm64' }
    default   { throw "Unsupported arch: $Arch" }
}

# ---------------------------------------------------------------------------
# Observability: transcript log + optional line-level tracer
# ---------------------------------------------------------------------------
# Default log path uses the version + arch so concurrent builds don't clobber
# each other. -LogFile '' (empty string) or $null disables it. Stop-Transcript
# runs in a finally{} so the log is closed even when the script throws.
if (-not $PSBoundParameters.ContainsKey('LogFile')) {
    $LogFile = Join-Path $DistDir "build-msix-$Version-$DistArch.log"
}
$transcriptStarted = $false
if ($LogFile) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $LogFile) | Out-Null
    try {
        Start-Transcript -Path $LogFile -Force | Out-Null
        $transcriptStarted = $true
        Write-Host "[INFO] Transcript: $LogFile" -ForegroundColor Cyan
    } catch {
        Write-Host "[WARN] Could not start transcript at $LogFile -- $_" -ForegroundColor Yellow
    }
}

# `-Trace` enables Set-PSDebug -Trace which prints each line as it runs:
#   1 = each statement (default; reasonable signal-to-noise)
#   2 = each statement plus every variable assignment (very chatty)
if ($Trace) {
    Write-Host "[INFO] Tracer enabled (level $TraceLevel) -- expect verbose output" -ForegroundColor Cyan
    Set-PSDebug -Trace $TraceLevel
}

# Always print the resolved parameter set up front so the transcript is
# self-contained. Pulled from PSBoundParameters so we capture defaults too.
Write-Host "[INFO] build-msix.ps1 -- v$Version, arch=$DistArch (manifest=$MsixArch), sign=$Sign" -ForegroundColor Cyan
Write-Verbose ("Parameters: " + ($PSBoundParameters | Out-String).Trim())
Write-Verbose "ProjectRoot: $ProjectRoot"
Write-Verbose "DistDir:     $DistDir"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
function Write-Info([string]$msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Write-Ok([string]$msg)   { Write-Host "[OK]   $msg" -ForegroundColor Green }
function Write-Warn([string]$msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Err([string]$msg)  { Write-Host "[ERR]  $msg" -ForegroundColor Red }

# Locate a Windows SDK tool under "Windows Kits\10\bin\<sdkver>\<arch>\".
# Picks the highest sdk version with a working x64 build of the tool.
function Find-SdkTool([string]$Name) {
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($root in @(
        'C:\Program Files (x86)\Windows Kits\10\bin',
        'C:\Program Files\Windows Kits\10\bin'
    )) {
        if (-not (Test-Path $root)) { continue }
        $candidate = Get-ChildItem -Path $root -Recurse -Filter $Name -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -like '*\x64\*' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($candidate) { return $candidate.FullName }
    }
    return $null
}

function Get-MakeAppxPath {
    if ($env:MAKEAPPX -and (Test-Path $env:MAKEAPPX)) { return $env:MAKEAPPX }
    return (Find-SdkTool 'makeappx.exe')
}
function Get-SignToolPath { return (Find-SdkTool 'signtool.exe') }

# ---------------------------------------------------------------------------
# Manifest validation. Catches the gotchas that MakeAppx surfaces only at
# pack time:
#   - XML well-formedness
#   - <Capabilities> children must be in xs:sequence order:
#       Capability -> uap*:Capability -> rescap*:Capability -> DeviceCapability
#   - Identity Version must be Major.Minor.Build.Revision
#   - ProcessorArchitecture must be x64|x86|arm64|arm|neutral
#   - runFullTrust required when any Application uses Windows.FullTrustApplication
# ---------------------------------------------------------------------------
function Test-AppxManifest([string]$Path) {
    if (-not (Test-Path $Path)) {
        Write-Err "Test-AppxManifest: no such file: $Path"
        return $false
    }
    $issues = 0

    # Use XmlDocument.Load(path) instead of [xml](Get-Content -Raw): Load()
    # reads the file as bytes and detects the encoding from the XML
    # declaration / BOM, while the [xml] cast on a -Raw string trips over a
    # leading UTF-8 BOM ("Syntax for an XML declaration is invalid. Line 1,
    # position 7.").
    try {
        $xml = New-Object System.Xml.XmlDocument
        $xml.Load($Path)
        Write-Info "AppxManifest.xml: well-formed XML"
    } catch {
        Write-Err "AppxManifest.xml: not well-formed XML -- $_"
        return $false
    }

    $nsm = New-Object System.Xml.XmlNamespaceManager $xml.NameTable
    $nsm.AddNamespace('a',      'http://schemas.microsoft.com/appx/manifest/foundation/windows10')
    $nsm.AddNamespace('uap',    'http://schemas.microsoft.com/appx/manifest/uap/windows10')
    $nsm.AddNamespace('rescap', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities')

    # Capabilities ordering
    $caps = $xml.SelectSingleNode('//a:Capabilities', $nsm)
    if ($caps) {
        $last = 0; $lastTag = ''
        foreach ($node in $caps.ChildNodes) {
            if ($node.NodeType -ne 'Element') { continue }
            $local = $node.LocalName
            $ns    = $node.NamespaceURI
            if     ($local -eq 'Capability'       -and $ns -like '*foundation/windows10' -and $ns -notlike '*restrictedcapabilities*') { $cls = 1; $tag = 'Capability' }
            elseif ($local -eq 'Capability'       -and $ns -like '*/uap*/windows10')       { $cls = 2; $tag = 'uap:Capability' }
            elseif ($local -eq 'Capability'       -and $ns -like '*restrictedcapabilities*') { $cls = 3; $tag = 'rescap:Capability' }
            elseif ($local -eq 'DeviceCapability'                                        ) { $cls = 4; $tag = 'DeviceCapability' }
            else { $cls = 99; $tag = $local }
            if ($cls -lt $last) {
                Write-Err "AppxManifest.xml: <Capabilities> children out of order"
                Write-Err "  Found <$tag> after <$lastTag>"
                Write-Err "  Required order (xs:sequence): Capability -> uap*:Capability -> rescap*:Capability -> DeviceCapability"
                $issues++
            }
            $last = $cls; $lastTag = $tag
        }
        if ($issues -eq 0) { Write-Info "AppxManifest.xml: <Capabilities> ordering ok" }
    }

    # Identity Version + ProcessorArchitecture
    $identity = $xml.SelectSingleNode('//a:Identity', $nsm)
    if (-not $identity) {
        Write-Err "AppxManifest.xml: <Identity> element missing"
        $issues++
    } else {
        $v = $identity.Version
        if ($v -notmatch '^\d+\.\d+\.\d+\.\d+$') {
            Write-Err "AppxManifest.xml: Identity Version must be 4-part (got '$v')"
            $issues++
        } else {
            Write-Info "AppxManifest.xml: Identity Version=$v"
        }
        $pa = $identity.ProcessorArchitecture
        if ($pa -notin @('x64','x86','arm64','arm','neutral')) {
            Write-Err "AppxManifest.xml: ProcessorArchitecture must be x64|x86|arm64|arm|neutral (got '$pa')"
            $issues++
        } else {
            Write-Info "AppxManifest.xml: ProcessorArchitecture=$pa"
        }
    }

    # runFullTrust if any Application uses Windows.FullTrustApplication
    $fullTrustApps = $xml.SelectNodes('//a:Application[@EntryPoint="Windows.FullTrustApplication"]', $nsm)
    if ($fullTrustApps.Count -gt 0) {
        $hasFullTrust = $xml.SelectSingleNode('//rescap:Capability[@Name="runFullTrust"]', $nsm)
        if (-not $hasFullTrust) {
            Write-Err "AppxManifest.xml: $($fullTrustApps.Count) Application(s) use Windows.FullTrustApplication"
            Write-Err "  but the package does not declare <rescap:Capability Name=`"runFullTrust`"/>"
            $issues++
        }
    }

    if ($issues -gt 0) {
        Write-Err "AppxManifest.xml: $issues validation issue(s) -- see above"
        return $false
    }
    return $true
}

# ---------------------------------------------------------------------------
# Main script body. Wrapped in try/finally so we always close the transcript
# and turn the tracer back off, even on a hard failure or exit.
# ---------------------------------------------------------------------------
try {

# ---------------------------------------------------------------------------
# Locate the build directory containing the .exe files.
# ---------------------------------------------------------------------------
if (-not $BuildDir) {
    $cargoTargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $ProjectRoot 'target' }
    foreach ($triple in @(
        'x86_64-pc-windows-gnullvm','x86_64-pc-windows-gnu','x86_64-pc-windows-msvc',
        'aarch64-pc-windows-gnullvm','aarch64-pc-windows-gnu','aarch64-pc-windows-msvc'
    )) {
        $cand = Join-Path $cargoTargetDir "$triple\release"
        if (Test-Path (Join-Path $cand 'sotf-tui.exe')) { $BuildDir = $cand; break }
    }
    if (-not $BuildDir) { $BuildDir = Join-Path $cargoTargetDir 'release' }
}
Write-Info "Build dir: $BuildDir"
$found = $false
foreach ($bin in @('sotf-desktop.exe','sotf-tui.exe')) {
    if (Test-Path (Join-Path $BuildDir $bin)) {
        Write-Info "  Found $bin"
        $found = $true
    }
}
if (-not $found) {
    Write-Err "No Windows binaries found in $BuildDir"
    Write-Info "Build them first with: cargo build --release -p sotf-tui --bin sotf-tui (etc.)"
    exit 1
}

# ---------------------------------------------------------------------------
# Stage
# ---------------------------------------------------------------------------
Write-Info "Building MSIX package v$Version ($DistArch)..."
$Staging = Join-Path $DistDir 'msix-staging'
$Output  = Join-Path $DistDir "sotf-desktop-$Version-windows-$DistArch.msix"

if (Test-Path $Staging) { Remove-Item -Recurse -Force $Staging }
New-Item -ItemType Directory -Force -Path "$Staging\assets" | Out-Null

foreach ($bin in @('sotf-desktop.exe','sotf-tui.exe')) {
    $src = Join-Path $BuildDir $bin
    if (Test-Path $src) {
        Copy-Item $src (Join-Path $Staging $bin)
        Write-Info "Added $bin"
    }
}
$nlopt = Join-Path $BuildDir 'nlopt.dll'
if (Test-Path $nlopt) { Copy-Item $nlopt (Join-Path $Staging 'nlopt.dll'); Write-Info "Added nlopt.dll" }

# Copy app assets (fonts, icons, headphone-targets -- not demo-audio)
$assetsSrc = Join-Path $ProjectRoot 'crates\app-gpui\assets'
if (Test-Path $assetsSrc) {
    foreach ($subdir in @('fonts','icons','headphone-targets')) {
        $src = Join-Path $assetsSrc $subdir
        if (Test-Path $src) { Copy-Item -Recurse $src "$Staging\assets\" }
    }
    $sotfPng = Join-Path $assetsSrc 'sotf.png'
    if (Test-Path $sotfPng) {
        foreach ($name in @('sotf-44x44.png','sotf-150x150.png','sotf-310x150.png')) {
            Copy-Item $sotfPng (Join-Path "$Staging\assets" $name)
        }
        Write-Info "Copied icon assets"
    }
}

# Render AppxManifest.xml. Substitute Identity Version and
# ProcessorArchitecture. The leading `\s` capture in the Version regex is
# critical: without it the substring `Version="..."` inside `MinVersion="..."`
# on <TargetDeviceFamily> also matches and corrupts that attribute (a sub-Win10
# MinVersion makes the package fail install).
# Read as raw UTF-8 bytes and strip any BOM(s) before doing string substitutions.
# `Get-Content -Raw` decodes the file using PS5.1's encoding heuristics, which
# can leave a U+FEFF char at the start of the string when the source has a BOM
# -- and then WriteAllText below re-encodes that as EF BB BF bytes, on top of
# any BOM the no-BOM encoder would otherwise omit, yielding a doubled BOM that
# fails XmlDocument.Load() with "Syntax for an XML declaration is invalid.
# Line 1, position 7." The fix: read bytes ourselves, strip leading BOM(s),
# decode as UTF-8.
$srcManifestPath = Join-Path $ProjectRoot 'builds\windows\AppxManifest.xml'
$srcBytes = [System.IO.File]::ReadAllBytes($srcManifestPath)
$bomLen = 0
while ($srcBytes.Length - $bomLen -ge 3 -and
       $srcBytes[$bomLen]   -eq 0xEF -and
       $srcBytes[$bomLen+1] -eq 0xBB -and
       $srcBytes[$bomLen+2] -eq 0xBF) {
    $bomLen += 3
}
$srcManifest = [System.Text.Encoding]::UTF8.GetString($srcBytes, $bomLen, $srcBytes.Length - $bomLen)

# Use -creplace (case-sensitive). PowerShell's default -replace is
# case-INSENSITIVE, which makes `Version="[^"]+"` also match the XML
# declaration's lowercase `version="1.0"` -- corrupting it to `Version="x.y.z.w"`
# and producing "Syntax for an XML declaration is invalid. Line 1, position 7."
# at Load() time. The source manifest uses capital V only on <Identity>, so
# case-sensitive matching disambiguates cleanly. Same for ProcessorArchitecture
# (only declared on <Identity>) -- kept -creplace for symmetry.
$rendered = $srcManifest `
    -creplace '(\s)Version="[^"]+"', "`$1Version=`"$MsixVersion`"" `
    -creplace 'ProcessorArchitecture="[^"]+"', "ProcessorArchitecture=`"$MsixArch`""

# Write WITHOUT a UTF-8 BOM. `Set-Content -Encoding UTF8` in Windows
# PowerShell 5.1 prepends a 3-byte BOM (EF BB BF), which then breaks
# `[xml](Get-Content -Raw $path)` because the BOM ends up as a leading
# string character and XmlDocument.LoadXml rejects it. MakeAppx is also
# pickier than the spec about a BOM in front of `<?xml` for AppxManifest.xml.
[System.IO.File]::WriteAllText(
    "$Staging\AppxManifest.xml",
    $rendered,
    (New-Object System.Text.UTF8Encoding($false))
)

# Pre-flight validation
if (-not (Test-AppxManifest "$Staging\AppxManifest.xml")) {
    Write-Err "Aborting MSIX build -- manifest validation failed."
    Remove-Item -Recurse -Force $Staging
    exit 1
}

# ---------------------------------------------------------------------------
# Optional signing of the staged exe files (before packaging)
# ---------------------------------------------------------------------------
$signtool = $null
if ($Sign) {
    $signtool = Get-SignToolPath
    if (-not $signtool) {
        Write-Err "SignTool.exe not found (Windows SDK)."
        exit 1
    }
    # Resolve cert inputs: CLI param wins over env var. Lets a remote driver
    # pass credentials as flags (which survive the SSH hop) while local users
    # can still rely on $env:WINDOWS_CERT_*.
    $cThumb = if ($CertThumbprint) { $CertThumbprint } else { $env:WINDOWS_CERT_THUMBPRINT }
    $cFile  = if ($CertFile)       { $CertFile }       else { $env:WINDOWS_CERT_FILE }
    $cPass  = if ($CertPassword)   { $CertPassword }   else { $env:WINDOWS_CERT_PASSWORD }
    $cTs    = if ($TimestampUrl)   { $TimestampUrl }   else { $env:WINDOWS_TIMESTAMP_URL }

    # Timestamping: by default use DigiCert's RFC3161 server. Set
    # -TimestampUrl 'none' (or WINDOWS_TIMESTAMP_URL='none') to skip /tr
    # entirely -- useful for offline signing, behind corporate proxies, or
    # to isolate timestamp-server failures during diagnosis. A signature
    # without a timestamp still works, it just expires when the cert expires.
    $signArgs = @('sign','/fd','SHA256')
    if (-not $cTs) { $cTs = 'http://timestamp.digicert.com' }
    if ($cTs -ne 'none') {
        $signArgs += @('/td','SHA256','/tr',$cTs)
    } else {
        Write-Warn "Timestamping disabled (TimestampUrl=none). Signature will expire when the cert expires."
    }
    if ($cThumb) {
        $signArgs += @('/sha1', $cThumb, '/sm')
    } elseif ($cFile) {
        if (-not (Test-Path $cFile)) {
            Write-Err "Cert file not found: $cFile"
            exit 1
        }
        $signArgs += @('/f', $cFile)
        if ($cPass) { $signArgs += @('/p', $cPass) }
    } else {
        Write-Err "Set -CertThumbprint / -CertFile (or `$env:WINDOWS_CERT_THUMBPRINT / `$env:WINDOWS_CERT_FILE) for signing."
        exit 1
    }
    foreach ($bin in @('sotf-desktop.exe','sotf-tui.exe')) {
        $p = Join-Path $Staging $bin
        if (Test-Path $p) {
            Write-Info "Signing $bin..."
            # Capture both streams so signtool's actual error text reaches the
            # transcript. Without 2>&1 the failure mode (wrong password, RFC3161
            # timestamp server unreachable, missing private key, etc.) is
            # invisible and only the wrapper "SignTool failed" line shows up.
            & $signtool @signArgs $p 2>&1 | ForEach-Object { Write-Host "  $_" }
            if ($LASTEXITCODE -ne 0) {
                Write-Err "SignTool failed for $bin (exit $LASTEXITCODE)"
                Write-Err "Common causes:"
                Write-Err "  - WINDOWS_CERT_PASSWORD wrong / cert key not exportable"
                Write-Err "  - Timestamp server unreachable (set `$env:WINDOWS_TIMESTAMP_URL='none' to skip /tr)"
                Write-Err "  - Cert lacks Code Signing EKU"
                Write-Err "  - SDK signtool too old (use signtool from a recent Windows 10/11 SDK)"
                exit 1
            }
        }
    }
}

# ---------------------------------------------------------------------------
# Pack
# ---------------------------------------------------------------------------
$makeappx = Get-MakeAppxPath
if (-not $makeappx) {
    Write-Err "MakeAppx.exe not found. Install the Windows SDK:"
    Write-Err "  https://developer.microsoft.com/windows/downloads/windows-sdk/"
    Write-Err "It then lives at C:\Program Files (x86)\Windows Kits\10\bin\<sdkver>\x64\makeappx.exe"
    exit 1
}
Write-Info "Packing with MakeAppx.exe: $makeappx"
if (Test-Path $Output) { Remove-Item -Force $Output }

& $makeappx pack /o /v /d $Staging /p $Output
if ($LASTEXITCODE -ne 0) {
    Write-Err "MakeAppx pack failed (exit $LASTEXITCODE)"
    exit $LASTEXITCODE
}

# ---------------------------------------------------------------------------
# Sign the .msix itself
# ---------------------------------------------------------------------------
if ($Sign) {
    Write-Info ("Signing " + (Split-Path -Leaf $Output) + "...")
    & $signtool @signArgs $Output
    if ($LASTEXITCODE -ne 0) { Write-Err "SignTool failed for .msix"; exit 1 }
    Write-Ok "MSIX signed."
}

Remove-Item -Recurse -Force $Staging

$size = "{0:N1} MB" -f ((Get-Item $Output).Length / 1MB)
Write-Ok "MSIX created: $Output  ($size)"
if (-not $Sign) {
    Write-Info ""
    Write-Info "Package is UNSIGNED. Re-run with -Sign to sign."
}

} finally {
    # Always tear down observability instrumentation, even on exit/throw.
    if ($Trace) { Set-PSDebug -Off }
    if ($transcriptStarted) {
        try { Stop-Transcript | Out-Null } catch { }
        Write-Host "[INFO] Transcript closed: $LogFile" -ForegroundColor Cyan
    }
}

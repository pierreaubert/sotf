@echo off
setlocal
REM Wrapper to run build-msix.ps1 under PowerShell 7 (pwsh) when available.
REM PS7's UTF-8-no-BOM default and modern .NET runtime avoid PS5.1 encoding
REM quirks (Get-Content -Raw heuristics, Set-Content BOM, etc.) and run faster.
REM Falls back to Windows PowerShell 5.1 (powershell.exe) if pwsh is missing,
REM with a one-line warning. All arguments are forwarded as-is.

pushd "%~dp0"

where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
    pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-msix.ps1" %*
) else (
    echo [WARN] pwsh ^(PowerShell 7+^) not found on PATH; falling back to Windows PowerShell 5.1. Install via "winget install Microsoft.PowerShell" for cleaner output.
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-msix.ps1" %*
)

set EXITCODE=%ERRORLEVEL%
popd
exit /b %EXITCODE%

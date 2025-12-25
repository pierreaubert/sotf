@echo off
REM Wrapper script to run build-windows.ps1
REM Usage: build-windows.bat [options]
REM   Options are passed directly to the PowerShell script

pushd "%~dp0"
powershell -ExecutionPolicy Bypass -File "%~dp0build-windows.ps1" %*
popd

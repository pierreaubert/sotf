# Building SotF GPUI for Windows using Docker

This guide explains how to build the Windows version of SotF GPUI using Docker on Linux.

## Prerequisites

- Docker installed
- KVM enabled (for hardware acceleration)
- At least 8GB RAM available for the VM
- ~40GB disk space for Windows + build tools

## Quick Start

### 1. Start Windows in Docker

```bash
# Create persistent storage for Windows and build cache
mkdir -p ~/docker-windows

# Run Windows 11 with the project mounted
docker run -it --rm --name windows -e "VERSION=11" \
  -p 8006:8006 \
  --device=/dev/kvm \
  --device=/dev/net/tun \
  --cap-add NET_ADMIN \
  -v "$HOME/docker-windows:/storage" \
  -e RAM_SIZE=64G \
  -e CPU_CORES=24 \
  --stop-timeout 120 \
  docker.io/dockurr/windows
```

### 2. Access Windows

**Option A - Web Browser (noVNC):**
Open http://localhost:8006 in your browser

**Option B - RDP Client:**
```bash
# Install an RDP client if needed
sudo apt install remmina

# Connect to localhost:3389
remmina -c rdp://localhost:3389
```

### 3. First-Time Setup (one-time)

Open PowerShell as Administrator and run:

```powershell
# Install Chocolatey (package manager)
Set-ExecutionPolicy Bypass -Scope Process -Force
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

# Install build tools
choco install -y visualstudio2022buildtools --package-parameters "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
choco install -y rust-ms
choco install -y llvm
choco install -y git

# Install vcpkg
cd C:\
git clone https://github.com/Microsoft/vcpkg.git
cd vcpkg
.\bootstrap-vcpkg.bat
.\vcpkg integrate install

# Install required libraries
.\vcpkg install openblas:x64-windows
.\vcpkg install nlopt:x64-windows

# Set environment variables (permanent)
[Environment]::SetEnvironmentVariable("VCPKG_ROOT", "C:\vcpkg", "Machine")
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", "C:\Program Files\LLVM\bin", "Machine")

# Restart PowerShell to pick up new environment variables
```

### 4. Build SotF GPUI

After setup, build the project:

```powershell
# Navigate to mounted project
cd Z:\sotf

# Run the build script
.\sotf-audio-player\windows\build-windows.ps1 -InstallDeps

# Or build just GPUI
.\sotf-audio-player\windows\build-windows.ps1 -GpuiOnly
```

The built binaries will be in `Z:\sotf\dist\`.

## Tips

### Persist Windows Installation

The first run downloads and installs Windows (~5-10GB). The installation is saved to `~/docker-windows/` so subsequent runs are faster.

### Increase Performance

```bash
# Use more RAM and CPU cores if available
docker run ... -e RAM_SIZE=16G -e CPU_CORES=8 ...
```

### Headless Builds (after initial setup)

Once Windows is set up with all tools, you can run builds via RDP/VNC without a GUI by using PowerShell remoting.

### Check KVM Support

```bash
# Verify KVM is available
ls -la /dev/kvm

# If not available, load the module
sudo modprobe kvm
sudo modprobe kvm_intel  # or kvm_amd for AMD CPUs
```

## Troubleshooting

### "KVM not available"

Ensure virtualization is enabled in BIOS and KVM modules are loaded:
```bash
sudo apt install qemu-kvm
sudo usermod -aG kvm $USER
# Log out and back in
```

### Slow Performance

- Increase RAM_SIZE and CPU_CORES
- Ensure KVM is working (not falling back to TCG emulation)
- Use SSD storage for ~/docker-windows

### Build Errors

- Ensure all environment variables are set correctly
- Try running `vcpkg integrate install` again
- Check that LLVM/libclang is installed properly

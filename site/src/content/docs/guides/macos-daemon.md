---
title: "macOS System-Wide Audio"
description: "Route all macOS system audio through SotF's plugin chain using the CoreAudio HAL driver."
---

SotF includes a CoreAudio HAL (Hardware Abstraction Layer) driver that creates a virtual
audio device. When you select this device as your system output, **all audio from all apps**
passes through your configured plugin chain before reaching your speakers or headphones.

This means your EQ, room correction, and upmixer apply system-wide — no per-app setup needed.

## Prerequisites

- macOS 13 (Ventura) or later
- Apple Silicon or Intel Mac
- SotF built with HAL support (`just prod-hal`)

## Installation

### 1. Build and Install the HAL Driver

```bash
just prod-hal       # Build the driver
just install-hal    # Install to ~/Library/Audio/Plug-Ins/HAL/
```

Alternatively, the installer package (`.pkg`) from the release page handles this automatically.

### 2. Restart CoreAudio

```bash
sudo killall coreaudiod
```

macOS restarts the audio daemon automatically. After a few seconds, you should see
**SotF Virtual Device** appear in System Settings → Sound.

### 3. Configure via Menubar App

Launch the menubar configuration app:

```bash
open -a SotFConfig
# or
cargo run --release --bin sotf-macos-configbar
```

The menubar app (speaker icon in the menu bar) lets you:
- Select which physical output device SotF routes audio to
- Choose your plugin preset
- Start and stop the SotF daemon
- View current processing status

### 4. Select the Virtual Device

In **System Settings → Sound → Output**, select **SotF Virtual Device**.

All system audio now flows: `App → SotF Virtual Device → Plugin Chain → Physical Device`.

## Starting the Daemon

The HAL driver requires the SotF processing daemon to be running. Start it:

```bash
sotf-daemon --config ~/.config/sotf/daemon.json
```

Or via the menubar app — click **Start Processing**.

To start automatically on login, add the daemon to your Login Items
(System Settings → General → Login Items) or use launchd:

```bash
# Install launchd plist
just install-daemon-launchd
```

## Switching Presets

From the menubar app, you can switch plugin presets without restarting the daemon.
The change applies in real time with a brief audio interruption.

From the command line:

```bash
sotf-daemon --reload-config
# or send SIGHUP to the running daemon process
kill -HUP $(pgrep sotf-daemon)
```

## Troubleshooting

### Virtual device not appearing in Sound settings

1. Check the driver is installed:
   ```bash
   ls ~/Library/Audio/Plug-Ins/HAL/
   ```
   You should see `SotF.driver`.

2. Restart CoreAudio:
   ```bash
   sudo killall coreaudiod
   ```

3. Check Console.app for `coreaudiod` errors mentioning SotF.

### No audio / silence after switching to virtual device

1. Verify the daemon is running: `ps aux | grep sotf-daemon`
2. Check the daemon's configured output device matches your physical output
3. Ensure no plugin in the chain has zeroed gain or is in a suspended state

### Audio crackles or dropouts

Increase the buffer size in the daemon config (`buffer_ms`). The default is tuned for
low latency; try 50–100 ms if your system is under load.

### Reverting to normal audio

Select your physical audio device in System Settings → Sound → Output.
The daemon continues running but is bypassed. You can stop it from the menubar app.

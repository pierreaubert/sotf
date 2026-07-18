# systemwide

System-wide audio processing subsystem for SOTF. Captures audio from the OS mixer, processes it through the plugin chain, and outputs to physical audio devices.

## Architecture

For a full component review, state-ownership analysis, and Mermaid use-case
diagrams, see [ARCHITECTURE.md](ARCHITECTURE.md).

```text
macOS Audio Apps (Safari, Spotify, ...)
         |
         v
  HAL Driver (virtual audio device, Swift)
         |
    shared memory (/tmp/sotf-{uid}/audio.shm)
         |
         v
  Daemon (Rust, sotf-daemon binary)
    - reads audio via AudioDriver trait
    - runs plugin chain (EQ, upmixer, compressor, ...)
    - writes to output device via cpal
         |
         v
  Physical speakers / headphones
```

External processes (Swift menubar app, GPUI configbar) control the daemon over a Unix domain socket with a JSON line protocol.

## Crates

| Crate | Lib name | Purpose |
|---|---|---|
| `driver-common` | `driver_common` | Platform-agnostic `AudioDriver` trait + `NullDriver` fallback |
| `driver-hal` | `driver_hal` | macOS CoreAudio HAL shared-memory bridge (encrypted via ChaCha20-Poly1305) |
| `daemon` | `sotf-daemon` | Background daemon binary; coordinates driver, engine, plugins, IPC |

### Platform support

- **macOS**: Full support via CoreAudio HAL driver (`--features hal`)
- **Linux**: PipeWire filter node (planned, `driver-common` trait is ready)
- **Windows**: APO + shared memory (planned)
- **Fallback**: `NullDriver` compiles everywhere, reports `platform_supported: false`

## Building

```bash
# macOS with HAL support
cargo build -p sotf-daemon --features hal --release

# Any platform (NullDriver fallback)
cargo build -p sotf-daemon --release
```

## Running

```bash
# Start the daemon
cargo run --bin sotf-daemon --features hal --release

# The daemon listens on a Unix socket at:
#   /tmp/sotf-{uid}/daemon.sock  (secure, default)
#   /tmp/autoeq_audio.sock       (legacy, SOTF_LEGACY_SOCKET=1)
```

### Local Lab

Run an isolated daemon without installing the HAL driver:

```bash
just systemwide-lab
```

This runs the daemon/state, real Unix-socket IPC, HAL protocol/streaming, and
Configbar model suites. The process-level scenarios start `sotf-daemon` with
`SOTF_SYSTEMWIDE_DRIVER=lab` in isolated temporary runtime directories. They
exercise coherent snapshots, 2 → 10 → 2 channel changes, transactional plugin
artifact rejection, shutdown, and restart without installing or touching the
CoreAudio HAL bundle.

## IPC protocol

JSON-over-Unix-socket, one object per line. Example:

Plugin artifacts may be linear racks or validated DAGs. Engine graph artifacts
use stable node IDs, explicit edges, channel counts, parameters, and bypass:

```json
{"command":"load_plugin_artifact","artifact":{"graph":{
  "nodes":[
    {"id":1,"plugin_type":"gain","parameters":{"gain_db":-3.0},"input_channels":2,"bypassed":false},
    {"id":2,"plugin_type":"eq","parameters":{"filters":[]},"input_channels":2,"bypassed":false}
  ],
  "edges":[{"from_node":1,"to_node":2}]
}}}
```

Graph candidates validate and apply transactionally. Configbar retains the rack
for linear topology and opens its graph editor for DAG topology.

```json
{"command": "load_plugins", "plugins": [
  {"plugin_type": "hal_input",  "parameters": {"channels": 2}},
  {"plugin_type": "eq",         "parameters": {"filters": [{"filter_type": "peak", "frequency": 1000, "q": 1.5, "gain_db": 3.0}]}},
  {"plugin_type": "hal_output", "parameters": {"channels": 2}}
]}
```

## Sub-crate documentation

Each sub-crate has its own README:

- [daemon/README.md](crates/daemon/README.md)
- [driver-hal/README.md](crates/driver-hal/README.md)
- [daemon/configbar/README_CONFIGURATION.md](crates/daemon/configbar/README_CONFIGURATION.md)

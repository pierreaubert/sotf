# SotF systemwide daemon

`sotf-daemon` owns the systemwide audio pipeline. It opens the platform driver,
processes captured frames through the daemon-owned plugin chain, and writes to
the selected physical output device. The SotF virtual device is an input
source only and must never be selected as the physical output sink.

## Runtime and IPC

The daemon uses synchronous engine operations behind a bounded, per-client
JSON-lines Unix-socket handler. The socket lives at the per-user runtime path
(`SOTF_SYSTEMWIDE_RUNTIME_DIR/daemon.sock`, or the platform-specific secure
fallback). Requests are authorized using peer credentials, and the instance
lock prevents two daemons from rotating the same session key or racing socket
startup.

The Configbar uses a serial background queue for mutations and a persistent,
reconnecting polling connection for status and metering. It adopts a reachable
daemon instead of terminating processes it did not launch.

## Useful commands

```bash
cargo run -p sotf-daemon --bin sotf-daemon
cargo test -p sotf-daemon
cargo check -p sotf-daemon
```

For deterministic local control/status tests, use `SOTF_SYSTEMWIDE_DRIVER=null`
or `SOTF_SYSTEMWIDE_DRIVER=lab` and point
`SOTF_SYSTEMWIDE_RUNTIME_DIR` at a temporary directory.

## Lifecycle and recovery

- `SIGINT` and `SIGTERM` clear the running flag, clear driver readiness, stop
  the engine, join the watcher, and remove only the daemon's verified socket.
- Pipeline mutations are serialized across IPC clients and driver-initiated
  reconfiguration.
- Failed pipeline applies attempt to restore the last applied working plan;
  status and snapshots expose recovery actions when restoration also fails.
- Sample-rate and buffer-size changes are accepted only while playback is
  idle. Channel changes use the shared-memory quiesce/configuration protocol.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) for the ownership model,
shared-memory protocol, encryption recovery, and diagnostic snapshot fields.

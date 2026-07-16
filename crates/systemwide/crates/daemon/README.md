# sotf-daemon

An audio player and recorder that supports audio plugins.

Background daemon for audio processing with plugin chain support. Includes an optional HAL feature for macOS system-wide audio integration.

The Unix daemon accepts synchronous JSON-line commands over a per-user socket.
A process-lifetime sibling lock serializes startup before stale-socket cleanup
and session-key rotation. Available-plugin metadata is initialized once and
reused across clients.

On Linux the daemon can run with the `NullDriver` or deterministic lab driver.
Windows currently validates only the portable `driver-common` contract; the
Unix daemon transport and a native Windows capture driver remain planned.

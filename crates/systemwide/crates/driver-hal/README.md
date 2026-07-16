# driver-hal

Shared memory interface for Swift HAL driver communication.

Rust side of the shared memory bridge used to exchange audio data with the macOS CoreAudio HAL driver. Communicates via memory-mapped file at `/tmp/sotf-{uid}/audio.shm`.

## Shared-memory contract

- Rust and Swift share a versioned C-layout header. Cross-process fields use
  matching acquire/release atomics, and tests pin the header size and offsets.
- The daemon owns geometry changes and raises the `configuring` gate while ring
  positions are reset. Swift re-checks that gate before publishing a position.
- Ring capacity is derived from the current header geometry, bounded by the
  mapped capacity.
- Encrypted IO uses ChaCha20-Poly1305 records and staging buffers preallocated
  for the maximum HAL geometry. Real-time entry points do not grow them.
- The daemon owns session-key rotation; shared-memory open/reinitialization
  never changes key files independently.

This crate is the macOS HAL bridge. Other platforms use `driver-common` and its
`NullDriver` fallback until native drivers are implemented.

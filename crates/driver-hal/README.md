# driver-hal

Shared memory interface for Swift HAL driver communication.

Rust side of the shared memory bridge used to exchange audio data with the macOS CoreAudio HAL driver. Communicates via memory-mapped file at `/tmp/sotf-{uid}/audio.shm`.

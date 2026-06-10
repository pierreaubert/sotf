# sotf-engine Architecture

This document complements `README.md` with implementation-level diagrams for
the playback runtime state machine.

## Playback Runtime State Machine

`run_playback_thread` in `src/engine/playback_thread/runtime.rs` constructs a
`PlaybackRuntime` and then runs a synchronous owner loop. The runtime uses
`std::thread`, `std::sync::mpsc`, atomics, `cpal`, and `rtrb`; it does not use
Tokio. Upstream processing sends `ProcessingMessage`s to the runtime, the
runtime owns the `rtrb::Producer`, and the CPAL output callback owns the
matching `rtrb::Consumer`.

Logical state is kept in small structs rather than in one public enum:

- `DrainState` tracks EOS, flush, drain start, and drain timeout.
- `RecoveryState` tracks callback progress, stream errors, retry timing,
  CoreAudio identity checks, and underrun milestones.
- `PlaybackAccounting` and `DiagnosticState` track frame counters and periodic
  stats emission.

```mermaid
stateDiagram-v2
    [*] --> BuildStream
    BuildStream --> Running: stream starts
    BuildStream --> Failed: device or stream build fails

    Running --> Running: SetVolume or Mute
    Running --> Rebuilding: UpdateSampleRate or UpdateChannels
    Rebuilding --> Running: rebuilt stream installed
    Rebuilding --> Running: old stream resumed after rebuild failure
    Rebuilding --> Stopped: old stream cannot resume

    Running --> DroppingUntilFlush: Stop command
    DroppingUntilFlush --> WaitingForDrain: Flush message observed
    WaitingForDrain --> Running: flush completed

    Running --> EosDrain: EndOfStream
    EosDrain --> PlaybackDrained: ring buffer empty
    PlaybackDrained --> Stopped: PlaybackDrained event sent
    EosDrain --> Stopped: drain timeout

    Running --> Recovery: stream error, callback stall, or device id change
    Recovery --> Running: recovered stream installed
    Recovery --> Running: previous stream resumed
    Recovery --> Stopped: unrecoverable recovery failure

    Running --> DisconnectDrain: upstream disconnect after EOS
    DisconnectDrain --> Stopped: drained or timed out
    Running --> Stopped: Shutdown or upstream disconnect

    Stopped --> [*]
    Failed --> [*]
```

## Runtime Loop

Each loop iteration is intentionally ordered. Commands are handled before new
audio, flush completion is observed before accepting more frames, and recovery
is checked before the runtime writes into the ring buffer.

```mermaid
flowchart TD
    Loop["PlaybackRuntime::run iteration"] --> CommandRx["try command_rx"]
    CommandRx -->|command| Command["handle_command"]
    CommandRx -->|empty| FlushDrain["wait_for_flush_drain"]
    Command -->|Break| Final["log final accounting"]
    Command -->|Proceed| FlushDrain

    FlushDrain -->|still draining| SleepFlush["sleep 1 ms"]
    SleepFlush --> Loop
    FlushDrain -->|ready| Recovery["handle_stream_recovery"]

    Recovery -->|retry wait or recovered| Loop
    Recovery -->|Break| Final
    Recovery -->|Proceed| Underrun["emit_underrun_milestone"]

    Underrun --> Space["has_minimum_ring_space"]
    Space -->|not enough room| Loop
    Space -->|enough room| Diagnostics["emit_periodic_diagnostics"]
    Diagnostics --> MessageRx["try message_rx"]

    MessageRx -->|Frame| Frame["handle_frame"]
    MessageRx -->|EndOfStream| EOS["handle_end_of_stream"]
    MessageRx -->|Flush| Flush["handle_flush"]
    MessageRx -->|Empty| Empty["handle_empty_queue"]
    MessageRx -->|Disconnected| Disconnected["handle_disconnected_queue"]

    Frame --> Loop
    EOS --> Loop
    Flush --> Loop
    Empty -->|sleep or keep draining| Loop
    Empty -->|drained or timeout| Final
    Disconnected --> Final
    Final --> Stop["Stopped"]
```

## Frame Hot Path

`write_frame_to_ring` in `src/engine/playback_thread/frame_writer.rs` is the
producer-side audio hot path. It must remain allocation-free after warmup:
conversion uses a preallocated `Vec<f32>`, direct writes use the incoming frame
buffer, and every path recycles the frame data exactly once. Logging, event
formatting, device lookup, and stream rebuilding stay outside this function.

```mermaid
flowchart TD
    Start["ProcessingMessage::Frame"] --> FlushDrop{"flush mode is DroppingUntilFlush?"}
    FlushDrop -->|yes| RecycleFlush["recycle frame data"]
    RecycleFlush --> DropCount["frames_dropped += 1"]

    FlushDrop -->|no| Received["frames_received += 1"]
    Received --> Channels{"frame channels == output channels?"}

    Channels -->|yes| DirectChunk["producer.write_chunk_uninit(frame samples)"]
    DirectChunk -->|ok| DirectCopy["write_chunk_bulk from frame.data"]
    DirectCopy --> DirectRecycle["recycle frame data"]
    DirectRecycle --> WrittenDirect["Written(samples = frame samples)"]
    DirectChunk -->|ring full| DropDirect["recycle frame data, sleep spin"]
    DropDirect --> Dropped["Dropped"]

    Channels -->|no| Capacity{"converted samples fit ring capacity?"}
    Capacity -->|no| DropConvertedTooLarge["recycle frame data, sleep spin"]
    DropConvertedTooLarge --> Dropped
    Capacity -->|yes| Buffer{"conversion buffer capacity is enough?"}
    Buffer -->|no| Invariant["ConversionBufferTooSmall"]
    Buffer -->|yes| Convert["clear and resize preallocated buffer; downmix, upmix, or copy"]
    Convert --> ConvertedChunk["producer.write_chunk_uninit(converted samples)"]
    ConvertedChunk -->|ok| ConvertedCopy["write_chunk_bulk from conversion buffer"]
    ConvertedCopy --> ConvertedRecycle["recycle frame data"]
    ConvertedRecycle --> WrittenConverted["Written(samples = converted samples)"]
    ConvertedChunk -->|ring full| DropConverted["recycle frame data, sleep spin"]
    DropConverted --> Dropped

    WrittenDirect --> AccountWritten["frames_written += 1; total_samples_written += samples"]
    WrittenConverted --> AccountWritten
    Dropped --> AccountDropped["frames_dropped += 1"]
    Invariant --> RateLimitedInvariant["rate-limit invariant event before formatting"]
```

## Stream Recovery

Recovery is part of the playback owner loop, not the CPAL callback. The callback
only updates atomics and uses the rate-limited stream-error log and event gates.
The owner loop later decides whether the stream needs rebuilding.

```mermaid
flowchart TD
    Tick["handle_stream_recovery"] --> Inputs["read stream_error_count, callback_count, frame counters"]
    Inputs --> CoreAudio{"CoreAudio identity check due?"}
    CoreAudio -->|yes| DeviceId["compare current output device id"]
    CoreAudio -->|no| Reason
    DeviceId --> Reason["playback_recovery_reason"]

    Reason -->|none| Running["continue running"]
    Reason -->|reason found| Retry{"retry interval elapsed?"}
    Retry -->|no| SleepRetry["sleep 10 ms"]
    SleepRetry --> Running
    Retry -->|yes| Drain["drain pending messages"]

    Drain --> Warn["send recovery warning event"]
    Warn --> Pause["pause current stream"]
    Pause --> Exclusive{"macOS exclusive access requested and inactive?"}
    Exclusive -->|yes| Reacquire["activate CoreAudio exclusive ownership"]
    Exclusive -->|no| Rebuild["rebuild_playback_stream"]
    Reacquire -->|ok| Rebuild
    Reacquire -->|error| Stop["emit ProcessingError and stop"]

    Rebuild -->|ok| Install["install_recovered_stream"]
    Install --> Reset["reset state, producer, conversion buffer, drain, and recovery counters"]
    Reset --> Running

    Rebuild -->|error| Resume["try to resume previous stream"]
    Resume --> Backoff["reset callback check; sleep 250 ms"]
    Backoff --> Running
```

## Logging And Event Boundaries

The playback state machine has two different logging domains:

- CPAL stream-error callbacks use `rate_limited_log!` and a separate
  `rate_limit::allow` gate before formatting or sending warning events.
- `write_frame_to_ring` has no logging or event construction in direct,
  converted, or full-buffer drop paths.

Lifecycle logs and warning events are allowed in the owner loop because stream
rebuilds, device lookup, final accounting, and recovery are outside the per-frame
write path.

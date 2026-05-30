---
title: Engine Configuration
description: Audio engine configuration reference.
---

`EngineConfig` is the serialized policy for the audio engine. Runtime-only
fields such as `output_device`, `sink_type`, `config_path`, and `watch_config`
are skipped during serialization; callers set them directly when constructing
the engine.

## Core Fields

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `version` | `u32` | `2` | Config schema version. Older files are migrated on load. |
| `frame_size` | `usize` | `1024` | Processing block size in frames. A value of `0` is sanitized to `1024`. |
| `buffer_ms` | `u32` | `200` | Queue depth in milliseconds. |
| `output_sample_rate` | `u32` | `48000` | Target hardware/output rate. A value of `0` is sanitized to `48000`. |
| `input_channels` | `usize` | `2` | Source channel count. |
| `output_channels` | `usize` | `2` | Hardware/engine output channel count. |
| `plugins` | `PluginConfig[]` | `[]` | Initial plugin chain. |
| `volume` | `f32` | `1.0` | Linear gain. |
| `muted` | `bool` | `false` | Initial mute state. |
| `driver_mode` | `bool` | `false` | Use a platform audio driver source instead of the file decoder. |
| `allow_virtual_output` | `bool` | `false` | Allows loopback/virtual devices for QA and explicit routing. |
| `latency_compensation` | `LatencyCompensationMode` | `enabled` | Enables plugin latency compensation in transport timing. |
| `oversampling_policy` | `EngineOversamplingPolicy` | `plugin_preferred` | Lets alias-prone plugins request host oversampling, or forces 2x/4x. |
| `network_endpoint` | `NetworkEndpointConfig` | disabled | Configures network input/client or HTTP endpoint mode. |

## Output Access

`output_access` requests the output device access mode:

| Value | Behavior |
| --- | --- |
| `shared` | Use the normal shared system output path. |
| `exclusive_preferred` | Try exclusive/bit-exact access when the selected backend supports it; fall back to shared output otherwise. |
| `exclusive_required` | Fail engine startup if exclusive access cannot be activated. |

Runtime state reports the result as `output_access_status`:

| Status | Meaning |
| --- | --- |
| `shared` | Shared output is active. |
| `exclusive_pending` | A platform backend, such as CoreAudio, should try to acquire exclusive ownership during playback setup. |
| `exclusive_active` | Exclusive output is active. ASIO devices report active immediately; CoreAudio reports active after hog-mode ownership is acquired. |
| `fallback_shared` | Exclusive was preferred but unavailable, so shared output is active. |
| `unsupported` | Exclusive was required but this build/backend cannot provide it. |

Callers can ask `plan_output_access(mode, output_device)` before starting the
engine. The returned `OutputAccessPlan` includes the selected backend
(`shared_cpal`, `core_audio_hog_mode`, `asio`, or `ios_system_output`), the
initial status, and a user-facing reason when the request must fall back or fail.

## DSD Output

`dsd_output` controls DSD/SACD handling:

| Value | Behavior |
| --- | --- |
| `disabled` | Reject DSD containers. |
| `pcm_decode` | Decode supported DSD containers to PCM. |
| `dop_preferred` | Prefer DoP output, but fall back to PCM decode because current playback backends cannot carry bit-perfect DoP frames. |
| `dop_required` | Reject playback unless DoP output is available. This build reports `dop_unavailable`. |
| `native_preferred` | Prefer native DSD output, but fall back to PCM decode because current playback backends cannot carry native DSD frames. |
| `native_required` | Reject playback unless native DSD output is available. This build reports `native_unavailable`. |

The current decoder capability surface is:

| Container | Capability |
| --- | --- |
| DSF | PCM decode available. |
| DFF/DSDIFF | PCM decode available for uncompressed DSD. DST-compressed DFF is rejected explicitly. |
| SACD ISO | Recognized, but not decoded. Extract DSF tracks or convert to PCM first. |

Runtime state reports this as `dsd_output_status`, with distinct fallback and
unavailable states so UI can tell “playing via PCM fallback” from “required
bitstream output is impossible.”

Callers can also ask `plan_dsd_output(mode)` before opening a source. The
returned `DsdOutputPlan` reports the selected backend (`disabled`, `pcm_decoder`,
`dop_bitstream`, or `native_bitstream`), the runtime status, and the fallback or
failure reason for preferred/required DoP and native DSD modes.

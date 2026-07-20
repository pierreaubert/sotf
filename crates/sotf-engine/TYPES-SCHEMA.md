# Engine types schema stability guide

This document lists the persisted JSON/config schemas owned by `sotf-engine` and
classifies each field as **stable** (safe for external tools to read or write)
or **internal** (may change without a migration window).

For migration rules, see `TYPES-CHANGELOG.md`.

## `EngineConfig`

Persisted as JSON by `EngineConfig::save_to_file` / `load_from_file`.
Current version: `2` (`default_engine_config_version`).

| Field | Stability | Notes |
|-------|-----------|-------|
| `version` | stable | Migration discriminator. Older versions (`0`, `1`) are upgraded on load. |
| `frame_size` | stable | Core audio block size. |
| `buffer_ms` | stable | Playback queue buffer length. |
| `output_sample_rate` | stable | Target hardware sample rate. |
| `input_channels` | stable | Decoder/source channel count. |
| `output_channels` | stable | Target output channel count. |
| `plugins` | stable | `Vec<PluginConfig>`; engine plugin chain. |
| `volume` | stable | Linear volume, range `0.0..=1.0`. |
| `muted` | stable | Mute toggle. |
| `driver_mode` | stable | Alias `hal_mode` accepted for backward compatibility. |
| `allow_virtual_output` | stable | Test-only virtual device flag. |
| `latency_compensation` | stable | Added in v2; defaults to `enabled`. |
| `output_access` | stable | Added in v2; defaults to `shared`. |
| `dsd_output` | stable | Added in v2; defaults to `disabled`. |
| `oversampling_policy` | stable | Added in v2; defaults to `plugin_preferred`. |
| `network_endpoint` | stable | Added in v2; defaults to disabled. |
| `output_device` | internal | Marked `#[serde(skip)]`; resolved at runtime, not persisted. |
| `config_path` | internal | Marked `#[serde(skip)]`; runtime bookkeeping only. |
| `watch_config` | internal | Marked `#[serde(skip)]`; runtime bookkeeping only. |
| `sink_type` | internal | Marked `#[serde(skip)]`; runtime bookkeeping only. |

Compatibility behavior:
- Unknown fields are ignored by serde.
- Missing fields with `#[serde(default)]` use their default value.
- Legacy versions are migrated forward and rewritten to disk.
- Versions newer than the current supported version are rejected.

## `PluginConfig`

Persisted inside `EngineConfig.plugins` and plugin preset files.

| Field | Stability | Notes |
|-------|-----------|-------|
| `plugin_type` | stable | Non-empty plugin identifier. |
| `parameters` | stable | Opaque plugin-specific JSON object. |

## `PluginGraphConfig`

Persisted in plugin graph presets.

| Field | Stability | Notes |
|-------|-----------|-------|
| `nodes` | stable | `Vec<PluginGraphNodeConfig>`. |
| `edges` | stable | `Vec<PluginGraphEdgeConfig>`. |
| `nodes[].id` | stable | Unique node ID. |
| `nodes[].plugin_type` | stable | Plugin identifier. |
| `nodes[].parameters` | stable | Plugin-specific JSON object. |
| `nodes[].input_channels` | stable | Must be greater than `0`. |
| `edges[].from_node` | stable | Source node ID. |
| `edges[].to_node` | stable | Target node ID. |

## `AudioEngineState`

Serialized for server status APIs and snapshots; not usually persisted to disk
as a primary config file, but clients may cache it.

| Field | Stability | Notes |
|-------|-----------|-------|
| `playback_state` | stable | `stopped` / `playing` / `paused`. |
| `current_source` | stable | `AudioSource` enum. |
| `current_file` | stable | Convenience path for `File` sources. |
| `position` | stable | Playback position in seconds. |
| `duration` | stable | Total duration if known. |
| `sample_rate` | stable | Current sample rate. |
| `num_channels` | stable | Current channel count. |
| `volume` | stable | Linear volume. |
| `muted` | stable | Mute toggle. |
| `processing_bypassed` | stable | Plugin bypass toggle. |
| `underruns` | stable | Cumulative underrun counter. |
| `plugin_latency_samples` | stable | Reported plugin chain latency. |
| `playback_output_device` | stable | Added with `#[serde(default)]`. |
| `playback_callback_count` | stable | Added with `#[serde(default)]`. |
| `playback_buffer_fill_percent` | stable | Added with `#[serde(default)]`. |
| `playback_stream_error_count` | stable | Added with `#[serde(default)]`. |
| `playback_frames_received` | stable | Added with `#[serde(default)]`. |
| `playback_frames_written` | stable | Added with `#[serde(default)]`. |
| `playback_frames_dropped` | stable | Added with `#[serde(default)]`. |
| `playback_effective_sample_rate` | stable | Added with `#[serde(default)]`. |
| `latency_compensation_enabled` | stable | Added with `#[serde(default)]`. |
| `output_access_mode` | stable | Added with `#[serde(default)]`. |
| `output_access_status` | stable | Added with `#[serde(default)]`. |
| `dsd_output_mode` | stable | Added with `#[serde(default)]`. |
| `dsd_output_status` | stable | Added with `#[serde(default)]`. |
| `oversampling_policy` | stable | Added with `#[serde(default)]`. |
| `network_endpoint` | stable | Added with `#[serde(default)]`. |
| `network_endpoint_status` | stable | Added with `#[serde(default)]`. |
| `stream_metadata` | stable | Added with `#[serde(default)]`. |
| `last_error` | stable | Last error message if any. |
| `seeking` | stable | Seek-in-progress flag. |
| `isolated_external_plugin_worker_statuses` | stable | Added with `#[serde(default)]`. |

Compatibility behavior:
- Unknown fields are ignored.
- All runtime-added counters and status fields default to `0` / `None` /
  their enum default when missing.

## `AudioSource`

Serialized as part of `AudioEngineState` and queue entries.

| Variant | Stability | Notes |
|---------|-----------|-------|
| `File` | stable | Single path string. |
| `Url` | stable | Object with `url`, `format_hint`, `seekable`. |
| `ServiceStream` | stable | Object with `service` (`spotify`/`tidal`) and `track_id`. |
| `Driver` | stable | String variant `driver`. |

## Version policy

- `EngineConfig` carries an explicit `version` field and a migration path.
- `PluginConfig`, `PluginGraphConfig`, and `AudioEngineState` rely on serde's
  default handling: additive fields must be `#[serde(default)]`, and removed
  fields must be tolerated as unknown.
- Breaking changes to stable fields require a version bump and a changelog
  entry documenting the migration path.

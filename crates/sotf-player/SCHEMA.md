# sotf-player schema stability guide

This document lists the persisted JSON/config schemas in `sotf-player` and
classifies each field as **stable** (safe for external tools to read or write)
or **internal** (may change without a migration window).

For migration rules, see the crate-level `CHANGELOG.md`.

## `AppConfig`

Persisted as the TUI app state (`~/.config/sotf/state.json` on Linux).
Current version: `1` (`default_app_config_version`).

| Field | Stability | Notes |
|-------|-----------|-------|
| `version` | stable | Migration discriminator. Versions below `1` are rejected. |
| `output_device` | stable | Selected output device name. |
| `queue` | stable | Queue of `(artist, album)` tuples. |
| `queue_index` | stable | Current position in queue. |
| `track_index` | stable | Current track index in the current album. |
| `plugin_preset` | stable | Last loaded plugin preset name. |

Compatibility behavior:
- Unknown fields are ignored by serde.
- Missing fields without `#[serde(default)]` will fail deserialization.
- Version `0` configs are rejected with a clear error; callers must recreate.

## `ServerConfig`

Persisted as `~/.config/sotf/servers.json`.

| Field | Stability | Notes |
|-------|-----------|-------|
| `mpd` | stable | `MpdSettings`. |
| `dlna` | stable | `DlnaSettings`. |
| `api` | stable | `SotfApiSettings`. |

All sub-fields are stable. Defaults are applied when a section is missing.

## `SourceConnectionConfig`

Persisted per source in the `library_sources.config_json` column and in
federation source entries.

| Variant | Stability | Notes |
|---------|-----------|-------|
| `Subsonic` | stable | `url`, `username`, `password`, `legacy_auth`. |
| `Mpd` | stable | `host`, `port` (default 6600), `auth_mode`, `password`, `httpd_port` (default 6601). |
| `Dlna` | stable | `location_url`, `friendly_name`. |
| `Peer` | stable | `host`, `port` (default 8732), `accepted_fingerprint`, `auth_token`. |
| `Tidal` | stable | `access_token`, `quality` (default `LOSSLESS`), `country_code` (default `US`). |
| `Spotify` | stable | `username`, `password`, `quality` (default `High`). |
| `IcyRadio` | stable | `url`, `name`. |

Compatibility behavior:
- Unknown fields are ignored.
- Fields with `#[serde(default)]` use their default when missing.
- The `type` tag discriminates variants; missing or unknown variants fail
  deserialization.

## `SotfRemoteServerStore`

Persisted as `~/.config/sotf/remote_servers.json`. Bearer tokens are **not**
stored here; they live in the separate token store or system keychain.

| Field | Stability | Notes |
|-------|-----------|-------|
| `version` | stable | Defaults to current version when missing. |
| `selected_server_id` | stable | ID of the currently selected server. |
| `servers` | stable | `Vec<SotfRemoteServer>`. |

| `SotfRemoteServer` field | Stability | Notes |
|--------------------------|-----------|-------|
| `id` | stable | Derived from normalized `api_base_url`. |
| `friendly_name` | stable | Display name. |
| `api_base_url` | stable | Normalized API base URL. |
| `origin_url` | stable | Origin URL for pairing/Web. |
| `host_name` | stable | Discovered host name. |
| `address` | stable | Discovered IP address. |
| `port` | stable | API port. |
| `protocol` | stable | `http` or `https`. |
| `api_path` | stable | API path, typically `/api/v1`. |
| `auth` | stable | Auth scheme, typically `bearer`. |

## `SotfRemoteTokenStore`

Persisted as `~/.config/sotf/remote_server_tokens.json` on platforms without a
system credential store. File permissions are owner-only on Unix.

| Field | Stability | Notes |
|-------|-----------|-------|
| `version` | stable | Defaults to current version when missing. |
| `tokens` | internal | Key/value map of bearer tokens. Debug output is redacted. |

## `MetadataServicesConfig`

Persisted as `~/.config/sotf/metadata_services.json`.

| Field | Stability | Notes |
|-------|-----------|-------|
| `providers` | stable | `Vec<MetadataProviderConfig>`. |
| `user_agent` | stable | HTTP user agent string. |

| `MetadataProviderConfig` field | Stability | Notes |
|-------------------------------|-----------|-------|
| `provider_id` | stable | Provider identifier, e.g. `musicbrainz`. |
| `enabled` | stable | Whether the provider is active. |
| `endpoint` | stable | Provider API endpoint. |
| `username` | stable | Optional username. |
| `has_stored_credentials` | stable | Flag indicating credentials are stored. |

## `RoomEqOptimizerConfig`

Persisted in Room EQ preset/state files and round-tripped through the
`autoeq::roomeq::OptimizerConfig` backend.

Recent breaking change (0.5.122): the legacy `target_tilt` and
`broadband_target_matching` fields were replaced by a unified
`target_response: TargetResponseUiConfig`. Legacy files still deserialize
because unknown fields are ignored, but the tilt/broadband-matching values are
not migrated and `target_response` falls back to defaults.

| Field | Stability | Notes |
|-------|-----------|-------|
| `mode` | stable | `Iir` / `Fir` / `Mixed` / `MixedPhase`. |
| `fir` | stable | FIR configuration. |
| `multi_speaker_mode` | stable | `Sequential` / `Combined`. |
| `algorithm` | stable | Optimizer algorithm identifier. |
| `strategy` | stable | DE strategy. |
| `de_f`, `de_cr` | stable | Differential-evolution parameters. |
| `adaptive_weight_f`, `adaptive_weight_cr` | stable | Adaptive weight parameters. |
| `spacing_weight`, `min_spacing_oct` | stable | Filter-spacing controls. |
| `sample_rate` | stable | Target sample rate. |
| `num_filters` | stable | Number of PEQ filters. |
| `min_q`, `max_q` | stable | Q bounds. |
| `min_db`, `max_db` | stable | Gain bounds. |
| `min_freq`, `max_freq` | stable | Frequency bounds. |
| `max_iter`, `population` | stable | Optimizer budget. |
| `peq_model` | stable | PEQ model. |
| `bo_*` | stable | Bayesian-optimization parameters. |
| `refine`, `local_algo` | stable | Refinement settings. |
| `loss_type` | stable | Loss function identifier. |
| `psychoacoustic`, `asymmetric_loss` | stable | Psychoacoustic toggles. |
| `smooth`, `smooth_n`, `tolerance`, `atolerance` | stable | Smoothing/convergence controls. |
| `target_curve`, `system_type` | stable | Target curve / system type strings. |
| `allow_delay`, `seed` | stable | Delay/seed options. |
| `vog` | stable | Voice-of-God config. |
| `mixed_config`, `mixed_phase` | stable | Mixed-mode / mixed-phase configs. |
| `target_response` | stable | **Current** target shaping config. |
| `excursion_protection` | stable | Excursion protection config. |
| `schroeder_split` | stable | Schroeder split config. |
| `phase_alignment` | stable | Phase alignment config. |
| `multi_seat` | stable | Multi-seat config. |
| `multi_measurement` | stable | Multi-measurement config. |
| `sub_config` | stable | Sub-optimizer config. |
| `channel_matching` | stable | Channel-matching config. |
| `epa_temporal_masking` | stable | EPA temporal-masking config. |
| `imported_from_file` | internal | UI state flag. |
| `target_tilt` | **removed** | Legacy field; ignored on read, never written. |
| `broadband_target_matching` | **removed** | Legacy field; ignored on read, never written. |

## `RecordingDeviceConfig` / `PlaybackDeviceConfig`

Persisted in Room EQ measurement metadata and microphone presets.

| `RecordingDeviceConfig` field | Stability | Notes |
|------------------------------|-----------|-------|
| `device_id` | stable | Device identifier. |
| `device_name` | stable | Human-readable device name. |
| `num_channels` | stable | Channel count. |
| `sample_rate` | stable | Sample rate. |
| `available_sample_rates` | stable | Supported sample rates. |
| `channel_mappings` | stable | Physical-to-logical channel map. |
| `mic_calibration_paths` | stable | Added with `#[serde(default)]`. |
| `num_positions` | stable | Added with `#[serde(default = "1")]`. |
| `ctc_matrix_strategy` | stable | Added with `#[serde(default)]`. |
| `ctc_loopback_input_channel` | stable | Added with `#[serde(default)]`. |

| `PlaybackDeviceConfig` field | Stability | Notes |
|-----------------------------|-----------|-------|
| `device_id` | stable | Device identifier. |
| `device_name` | stable | Human-readable device name. |
| `num_channels` | stable | Channel count. |
| `sample_rate` | stable | Sample rate. |
| `available_sample_rates` | stable | Supported sample rates. |
| `speaker_configuration` | stable | Speaker layout enum. |
| `channel_mappings` | stable | Channel mapping array. |

## `MicrophonePresetsConfig`

Persisted as `~/.config/sotf/microphone_presets.json`.

| Field | Stability | Notes |
|-------|-----------|-------|
| `presets` | stable | `Vec<MicrophonePreset>`. |

| `MicrophonePreset` field | Stability | Notes |
|--------------------------|-----------|-------|
| `name` | stable | Preset name. |
| `device_name` | stable | Device name. |
| `channel_mappings` | stable | Physical input channels. |
| `mic_calibration_paths` | stable | Calibration path per channel. |

## Version policy

- `AppConfig` carries an explicit `version` and rejects unsupported older
  versions.
- All other persisted types rely on serde default handling: additive fields
  must be `#[serde(default)]`, and removed fields must be tolerated as unknown.
- Breaking changes to stable fields require a changelog entry and, where
  possible, a regression test in the corresponding `tests.rs`.

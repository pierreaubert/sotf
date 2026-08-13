# Binaural Decoder — UI Specification

The generated UI is defined by `src/params.rs`; this document mirrors that
canonical schema. `sofa_file` is the only public SOFA key. Legacy construction
state using `hrtf_file` is accepted as an alias but is not exposed twice.

| Index | Key | Control | Range/default | Update |
|---:|---|---|---|---|
| 0 | `sofa_file` | file picker | empty | setup |
| 1 | `input_channels` | read-only | exact supported layout | structural |
| 2 | `externalization` | knob | 0–1 / 0 | realtime |
| 3 | `near_field_strength` | knob | 0–1 / 0 | structural |
| 4 | `crossfade_mode` | selector | Linear, Spectral / Linear | realtime |
| 5 | `late_reverb_enabled` | toggle | false | realtime |
| 6 | `late_reverb_mix` | knob | 0–1 / 0.3 | realtime |
| 7 | `late_reverb_rt60` | knob | 0.1–5 s / 1 s | realtime |
| 8 | `late_reverb_damping` | knob | 0–1 / 0.3 | realtime |
| 9 | `crossfade_ms` | knob | 10–500 ms / 50 ms | realtime |
| 10 | `head_yaw_deg` | knob | ±180° / 0° | realtime |
| 11 | `head_pitch_deg` | knob | ±180° / 0° | realtime |
| 12 | `head_roll_deg` | knob | ±180° / 0° | realtime |
| 13 | `hrtf_database_dir` | directory picker | empty | setup |
| 14 | `head_width_cm` | knob | 10–25 cm / 15 cm | structural |
| 15 | `ear_height_cm` | knob | 4–16 cm / 10 cm | structural |

Supported input widths are `1, 2, 3, 5, 6, 8, 10, 12, 14, 16`; ambiguous
counts such as four channels are rejected rather than mapped silently to stereo.

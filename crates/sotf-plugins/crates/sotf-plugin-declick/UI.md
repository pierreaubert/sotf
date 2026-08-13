# Declick UI

The Repair group exposes all live parameters:

| Control | Type | Range/default | Behavior |
| --- | --- | --- | --- |
| Enabled | Toggle | On | 5 ms crossfade between delayed dry and repaired audio; detector stays warm. |
| Sensitivity | Knob | 1–100 / 10 | Lower values repair more candidates; automation is smoothed over 5 ms. |
| Link Channels | Toggle | On | Share detection decisions in adjacent channel pairs while interpolating each channel separately. |

The host should display eight samples of latency in both enabled and disabled
states. There is no dynamic visualization or analyzer output.

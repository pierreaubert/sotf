# 0.1.36

## Driver HAL streaming configuration

- Driver playback startup now reads the HAL driver's reported sample rate and
  channel count before starting the engine, falling back to 48 kHz stereo only
  when the driver has not reported a format yet.
- Driver reconfiguration now passes the negotiated HAL sample rate into the
  engine restart path instead of restarting through the default 48 kHz HAL
  helper.
- Sample-rate and buffer-frame commands now rely on an acknowledged HAL config
  request; if the HAL does not apply the change, `driver-hal` returns an error
  instead of reporting success optimistically.
- Reconfiguration now preserves the HAL input channel count when available and
  uses the explicit driver-format engine startup helper.
- Added regression coverage through `driver-hal`'s streaming guard tests to
  ensure the negotiated sample rate is wired into `reconfigure_audio_pipeline`.

Verified:
- `cargo test -p sotf-daemon`
- `cargo check -p sotf-daemon -p driver-hal -p sotf-engine --features
  sotf-daemon/hal,sotf-engine/hal`

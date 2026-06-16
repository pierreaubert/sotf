# sotf-plugin-stereo-imager

Stereo Imager — multi-band M/S stereo width control.

## Architecture

- `lib.rs` — Main `StereoImagerPlugin`, implements `ParametricParametricInPlacePlugin` trait, `StereoImagerPluginParams`
- `params.rs` — Parameter definitions

## Key Public API

- `StereoImagerPlugin` implementing `ParametricParametricInPlacePlugin`
- Wrapped with `ParametricParametricInPlacePluginAdapter` for host use

## Testing

```bash
cargo test -p sotf-plugin-stereo-imager
```

## Important Notes

- Stereo only (2 channels)
- M/S (Mid/Side) encoding for width control
- Multi-band: different width settings per frequency band
- ParametricParametricInPlacePlugin — processes stereo buffer in-place via the parametric adapter

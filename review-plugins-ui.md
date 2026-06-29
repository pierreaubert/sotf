# GPUI Plugin UI Review — `crates/app-gpui/components/plugins`

**Scope:** Static code review of every plugin surface rendered by the GPUI app.  
**Date:** 2026-06-29  
**Method:** Compared each plugin's parameter specs / `PluginLayout` (or `GLOBAL_PARAMS` / `BAND_TEMPLATE`) against the registered custom view or the automatic `ui_layout_renderer` output.

## How UI routing works

`crates/app-gpui/components/plugins/mod.rs::render_plugin_content` picks the renderer in this order:

1. **Custom view** — if the plugin type key is in `custom_view_registry/gpui_view_registry.rs`.
2. **Automatic layout** — otherwise, if `PluginSettings::layout()` returns `Some`.
3. **Empty surface** — otherwise, a blank `div` is rendered.

A plugin can have a hand-written UI file that is **not registered**, in which case it is dead code and the automatic layout (or nothing) is what the user actually sees.

---

## Summary table

| Plugin | Type key | UI kind | Status | Main problem |
|--------|----------|---------|--------|--------------|
| EQ | `eq` | Custom | Major gaps | Global params `max_filters`, `tdf2`, `topology` read-only; per-band `topology` not in spec |
| Dynamic EQ | `dynamic_eq` | Custom | OK | — |
| Linear-Phase EQ | `linear_phase_eq` | Custom (shared) | Major gaps | Global params read-only; per-band `active` missing; 4-stride hardcoded for 5-param band template |
| FIR Designer | `fir_designer` | Custom (shared) | Major gaps | Same stride/global-param issues as Linear-Phase EQ |
| Spectrum Analyzer | `spectrum_analyzer` | Custom | Minor issues | Knob ranges hardcoded, do not match `ParamSpec` |
| Channel Mute/Solo | `channel_mute_solo` | Custom | Major gaps | M/S/D buttons have no click handlers |
| Matrix | `matrix` | Custom | OK | Single `gain` param not exposed as a knob, but grid editing is sufficient |
| Loudness Monitor | `loudness_monitor` | Custom | OK | Analyzer, no params |
| Multiband Compressor | `multiband_compressor` | Custom | OK | `band_selector` viz slot unused, but band tabs replace it |
| Multiband Expander | `multiband_expander` | Custom | Minor issues | `detection_mode` rendered as 0/1 knob instead of selector |
| A/B Compare | `ab_compare` | Custom | Minor issues | `Paths` tab / file pickers from layout not rendered (custom Load buttons used instead) |
| Upmixer | `upmixer` | Custom | Major gaps | `speaker_config`, `binaural_preview`, `bandpass_hz`, and all output auto-gain params missing |
| Gain | `gain` | Automatic | Minor issues | Unregistered `ui_gain.rs` dead code; automatic layout is correct |
| Compressor | `compressor` | Automatic | OK | Uses single-band layout of multiband compressor crate |
| Limiter | `limiter` | Automatic | OK | — |
| Gate | `gate` | Automatic | OK | — |
| Expander | `expander` | Automatic | OK | Uses single-band layout of multiband expander crate |
| Loudness Compensation | `loudness_compensation` | Automatic | Minor issues | Mode selector is a `Selector`, so `detect_mode_selector` never filters ISO 226 / AUTO groups; `reference_level_db` shown twice |
| Fletcher-Munson | `fletcher_munson` | None | No UI | No layout or param specs wired for the settings variant |
| Delay | `delay` | Automatic | OK | — |
| Crossfeed | `crossfeed` | Automatic | OK | Mode selector filtering works |
| Convolution | `convolution` | Automatic | OK | IR file picker wired |
| Binaural Decoder | `binaural` | Automatic | OK | SOFA file picker wired |
| XTC | `xtc` | Automatic | Major gaps | `room_ir_file` field exists but no file picker in layout |
| AAE | `aae` | Automatic | OK | Spatial spider visualization rendered |
| Ambisonics Decoder | `ambisonics_decoder` | Automatic | OK | — |
| Downmix | `downmix` | Automatic | Minor issues | Unregistered `ui_downmix.rs` dead code with stale labels/omissions |
| Mono to Stereo | `mono_to_stereo` | Automatic | Minor issues | Unregistered `ui_mono_to_stereo.rs` dead code references removed params |
| Denoiser | `denoiser` | Automatic | OK | — |
| Speech Denoiser | `speech_denoiser` | Automatic | OK | — |
| Hiss Reducer | `hiss_reducer` | Automatic | OK | — |
| Declick | `declick` | Automatic | OK | — |
| PND | `pnd` | Automatic | OK | — |
| AEC | `aec` | Automatic | OK | Post-Filter lives in output/config popover only |
| Beamformer | `beamformer` | Automatic | OK | — |
| Spectral Compressor | `spectral_compressor` | Automatic | Minor issues | Layout OK; stale doc comment omits last two params |
| Stereo Imager | `stereo_imager` | Automatic | OK | — |
| De-Esser | `de_esser` | Automatic | Minor issues | GR meter rendered but not wired for `DeEsserData` |
| Transient Shaper | `transient_shaper` | Automatic | OK | — |
| Saturation | `saturation` | Automatic | OK | — |
| Band Split | `band_split` | Automatic | OK | — |
| Band Merge | `band_merge` | Automatic | OK | — |
| Dither | `dither` | Automatic | OK | — |
| Crossover | `crossover` | None | No UI | Not represented in `PluginSettings`, so no surface |

---

## Per-plugin details

### Custom UIs

#### EQ (`eq`) — Major gaps
- **Expected:** `GLOBAL_PARAMS`: `max_filters`, `tdf2`, `topology`; `BAND_TEMPLATE`: `filter_type`, `frequency`, `q`, `gain`.
- **Actual:** Frequency-response graph, draggable control points, band tabs with mute/solo, per-band Freq/Q/Gain knobs, filter-type button set, per-band topology pill, `+` add-band button.
- **Issues:**
  - The three global params are displayed as read-only header text (`Filters: …`, `Topology: …`, `TDF-II: …`) and cannot be edited.
  - A per-band topology control is rendered even though `BAND_TEMPLATE` has no `topology` field, so its parameter index mapping is undefined.
  - `max_filters` is only mutated indirectly by adding/removing bands.

#### Dynamic EQ (`dynamic_eq`) — OK
- **Expected:** 8 globals (`num_bands`, `threshold`, `ratio`, `attack`, `release`, `knee`, `link_channels`, `mix`) and 7 per-band params (`frequency`, `q`, `gain`, `band_threshold`, `band_ratio`, `active`, `solo`).
- **Actual:** Global knobs + Link toggle, clickable band tabs, per-band Freq/Q/Gain/Thresh/Ratio knobs, Active/Solo toggles.
- **Issues:** None — all params are exposed.

#### Linear-Phase EQ (`linear_phase_eq`) — Major gaps
- **Expected:** Globals `num_filters`, `fir_length`, `auto_gain`, `mix`; per-band `filter_type`, `frequency`, `q`, `gain_db`, `active`.
- **Actual:** Reuses the standard EQ graph + band controls.
- **Issues:**
  - `num_filters`, `fir_length`, `auto_gain`, `mix` are read-only text, not editable.
  - Per-band `active` toggle is not exposed.
  - `ui_eq/render.rs` hardcodes a band stride of `selected_band_idx * 4`, but the Linear-Phase/FIR band template has 5 params, so indices drift.

#### FIR Designer (`fir_designer`) — Major gaps
- **Expected:** Globals `num_filters`, `fir_length`, `phase_mode`, `auto_gain`, `mix`; per-band template same as Linear-Phase EQ.
- **Actual:** Reuses the standard EQ graph + band controls.
- **Issues:** Same as Linear-Phase EQ: globals read-only, `active` missing, 4-stride vs 5-param mismatch.

#### Spectrum Analyzer (`spectrum_analyzer`) — Minor issues
- **Expected:** `num_bins` 8–120, `min_freq` 10–100 Hz, `max_freq` 5000–22050 Hz, `smoothing` 0–1, `tilt_correction` choice, `tilt_reference` choice.
- **Actual:** Spectrum graph + knobs for Bins, Min Hz, Max Hz, Smooth + selectors for Tilt/Reference.
- **Issues:** Hardcoded knob ranges in `ui_spectrum.rs` do not match specs:
  - `num_bins` rendered 10–100 vs spec 8–120
  - `min_freq` rendered 10–1000 vs spec 10–100
  - `max_freq` rendered 1000–24000 vs spec 5000–22050

#### Channel Mute/Solo (`channel_mute_solo`) — Major gaps
- **Expected:** `enabled`, `dim_gain_db`, `fade_ms`, and per-channel M/S/D state.
- **Actual:** Setup toggles/knobs + per-channel strips with M/S/D buttons.
- **Issues:** The Mute, Solo, and Dim buttons are rendered as colored labels but have **no `on_mouse_down` / click handlers**, so users cannot change channel state from the UI.

#### Matrix (`matrix`) — OK
- **Expected:** Single scalar `gain` (0–1).
- **Actual:** Preset button set + interactive input×output gain grid + per-output M/S/D sidebar.
- **Issues:** The scalar `gain` param is not rendered, but the grid gives full control. Functional.

#### Loudness Monitor (`loudness_monitor`) — OK
- **Expected:** None (analyzer).
- **Actual:** LUFS / true-peak meter readout.
- **Issues:** None.

#### Multiband Compressor (`multiband_compressor`) — OK
- **Expected:** 16 globals + 10 per-band params.
- **Actual:** Global column, band tabs, DYNAMICS/TIMING sliders, transfer curve, per-band Active/Solo/Bypass/AutoGain, OUTPUT column.
- **Issues:** `VizSlot::Custom { name: "band_selector" }` from `LAYOUT.visualizations` is ignored, but the explicit band tabs provide the same functionality.

#### Multiband Expander (`multiband_expander`) — Minor issues
- **Expected:** 17 globals + 12 per-band params.
- **Actual:** Similar to compressor expander UI.
- **Issues:**
  - `detection_mode` is a Peak/RMS choice but rendered as a 0/1 knob instead of a selector.
  - `hysteresis` is placed in DYNAMICS rather than TIMING as declared in the layout (cosmetic).

#### A/B Compare (`ab_compare`) — Minor issues
- **Expected:** Main params (`mix`, `mix_mode`, `selected_path`, `bypass`, `difference_mode`, `mix_transition_ms`), phase invert toggles, auto-gain params, and a `Paths` tab with file pickers for `path_a_config`/`path_b_config`.
- **Actual:** Uses `render_main_controls_from_layout` for main groups, plus custom Path A/B sections with Load buttons, file-name display, and sub-plugin menus.
- **Issues:** The `Paths` tab from `LAYOUT` is not rendered; file loading is handled by the custom Load buttons instead. All other main params are present.

#### Upmixer (`upmixer`) — Major gaps
- **Expected:** Config `speaker_config`, `binaural_preview`; main `gain_front_direct`, `gain_front_ambient`, `gain_rear_ambient`, `height_gain`, `stereo_width`, `center_spread`, `bandpass_hz`; output `safety_cap_db`, `auto_gain_enabled`, `auto_gain_max_db`, `auto_gain_smoothing_ms`; plus 9 tabs of params.
- **Actual:** Vertical sliders for channel/spatial gains, 9-tab panel, permanent spatial-spider graph.
- **Issues:** Several declared params are missing from the custom view:
  - `speaker_config` selector
  - `binaural_preview` toggle
  - `bandpass_hz`
  - `auto_gain_enabled`, `auto_gain_max_db`, `auto_gain_smoothing_ms`
- The custom view also reorganizes tabs (`HR Direct` + `Decorrelation` + `Spatial` instead of a single `Enhancement`), which is a UX divergence rather than a bug.

---

### Automatic (layout-driven) UIs

#### Gain (`gain`) — Minor issues
- **Expected:** `gain_db`, `smoothing_ms`.
- **Actual:** Large knob + small knob from automatic layout.
- **Issues:** `ui_gain.rs` exists but is **not registered** in `GpuiViewRegistry`. If it were registered, it would also omit `smoothing_ms`.

#### Compressor (`compressor`) — OK
- **Expected:** Full single-band compressor layout from `sotf-plugin-multiband-compressor::SINGLE_BAND_LAYOUT`.
- **Actual:** All 16 params rendered by the layout renderer.
- **Issues:** None.

#### Limiter (`limiter`) — OK
- **Expected:** Threshold, release, lookahead, soft/true_peak/isp_mode/dual_release/feed_forward toggles, link_amount, mix.
- **Actual:** All rendered with labeled button sets and sliders.
- **Issues:** None.

#### Gate (`gate`) — OK
- **Expected:** Full gate param set including sidechain HPF order/detection selectors.
- **Actual:** All controls present; choice params render as dropdown selectors.
- **Issues:** None.

#### Expander (`expander`) — OK
- **Expected:** Full single-band expander layout from `sotf-plugin-multiband-expander::SINGLE_BAND_LAYOUT`.
- **Actual:** All 15 single-band params rendered, including `hysteresis` on the Advanced tab.
- **Issues:** None.

#### Loudness Compensation (`loudness_compensation`) — Minor issues
- **Expected:** Mode selector (Manual/ISO 226/Auto), mode-dependent groups, EQ bands, output auto-gain.
- **Actual:** Mode selector rendered as a `Selector` dropdown; all groups visible together.
- **Issues:**
  - `detect_mode_selector` only recognizes a `ButtonSet` in an untitled main group, so the ISO 226 and AUTO groups are never filtered by mode.
  - `reference_level_db` appears twice (under ISO 226 and under AUTO).

#### Fletcher-Munson (`fletcher_munson`) — No UI
- **Expected:** Legacy compat params.
- **Actual:** Blank surface.
- **Issues:** `PluginSettings::FletcherMunson` is in `no_params_struct`, so `layout()` returns `None` and no custom view is registered. Factory converts creation to `LoudnessCompensation`, but persisted `FletcherMunson` settings are not editable.

#### Delay (`delay`) — OK
- **Expected:** Delay/feedback/allpass/modulation params.
- **Actual:** Main sliders + Modulation tab.
- **Issues:** None.

#### Crossfeed (`crossfeed`) — OK
- **Expected:** Mode/preset toggles, mode-specific gain knobs, auto-gain, mix.
- **Actual:** Mode button-set filters Bauer/Meier/Multiband groups; all params exposed.
- **Issues:** None.

#### Convolution (`convolution`) — OK
- **Expected:** IR file picker, mix/gain, advanced toggles.
- **Actual:** File picker dispatches `OpenIrFile`, output knobs, Advanced tab.
- **Issues:** None.

#### Binaural Decoder (`binaural`) — OK
- **Expected:** SOFA file picker, input channels label, externalization, reverb params.
- **Actual:** File picker dispatches `OpenSofaFile`, read-only input channels, main knobs, Reverb tab.
- **Issues:** None.

#### XTC (`xtc`) — Major gaps
- **Expected:** Geometry, beta/shadow, room-reflection knobs and toggles.
- **Actual:** All declared `PARAMS`/`LAYOUT` controls rendered.
- **Issues:** `PluginSettings::XTC` carries a `room_ir_file: Option<String>` but it is not declared in `PARAMS` or `LAYOUT`, so there is no IR file picker for room reflections.

#### AAE (`aae`) — OK
- **Expected:** Room/levels/output/tabs + spatial spider visualization.
- **Actual:** All 25 params rendered; spatial spider shown FullCenter.
- **Issues:** None.

#### Ambisonics Decoder (`ambisonics_decoder`) — OK
- **Expected:** Order knob, target layout selector, max-rE/dual-band toggles.
- **Actual:** Exactly those controls.
- **Issues:** None.

#### Downmix (`downmix`) — Minor issues
- **Expected:** Phase coherence/ITU toggles, gain knobs, phase blend tab.
- **Actual:** Automatic layout renders all declared controls.
- **Issues:** `ui_downmix.rs` is unregistered dead code with stale label "Enable FFT Phase Alignment" and missing `itu_mode`.

#### Mono to Stereo (`mono_to_stereo`) — Minor issues
- **Expected:** Width, Haas delay, decorrelation, freq-dependent toggle.
- **Actual:** Automatic layout renders correctly.
- **Issues:** `ui_mono_to_stereo.rs` is unregistered dead code that still references removed params `enable_comp_eq` / `comp_eq_depth_db` and omits `freq_dependent`.

#### Denoiser (`denoiser`) — OK
- **Expected:** 29 params across main groups and tabs.
- **Actual:** All groups/tabs rendered by automatic layout.
- **Issues:** None.

#### Speech Denoiser (`speech_denoiser`) — OK
- **Expected:** Single `enabled` toggle.
- **Actual:** One Off/On toggle.
- **Issues:** None.

#### Hiss Reducer (`hiss_reducer`) — OK
- **Expected:** Enabled, threshold, frequency, strength.
- **Actual:** Toggle + knobs + slider.
- **Issues:** None.

#### Declick (`declick`) — OK
- **Expected:** Enabled, sensitivity.
- **Actual:** Toggle + large knob.
- **Issues:** None.

#### PND (`pnd`) — OK
- **Expected:** Correction/analysis knobs and toggles.
- **Actual:** All controls rendered.
- **Issues:** None.

#### AEC (`aec`) — OK
- **Expected:** Echo tail, step size, post-filter.
- **Actual:** Main sliders; post-filter toggle in output/config popover.
- **Issues:** Post-filter not on the main surface, but this matches the layout declaration.

#### Beamformer (`beamformer`) — OK
- **Expected:** Mic count/spacing in config, steer angle/algorithm in main.
- **Actual:** Algorithm selector with 3 labels matching `BEAMFORMER_TYPES`.
- **Issues:** None.

#### Spectral Compressor (`spectral_compressor`) — Minor issues
- **Expected:** FFT size, dynamics, timing, output, adaptive controls.
- **Actual:** All params rendered; config/output controls in popovers.
- **Issues:** Stale doc comment in `params.rs` omits `adaptive_threshold` and `adaptive_offset_db`; the layout itself is correct.

#### Stereo Imager (`stereo_imager`) — OK
- **Expected:** Width, crossover/width per band, mono bass, mix.
- **Actual:** All controls rendered.
- **Issues:** None.

#### De-Esser (`de_esser`) — Minor issues
- **Expected:** Detection/dynamics params, mode selector, mix, GR meter.
- **Actual:** Mode selector, sliders, output GR meter, mix.
- **Issues:** The GR meter is not wired for `DeEsserData`; `render_bar_meter` only downcasts `CompressorData`, `LimiterData`, and `GateData`, so the meter always reads 0 dB.

#### Transient Shaper (`transient_shaper`) — OK
- **Expected:** Attack, sustain, sensitivity, output gain, mix.
- **Actual:** All controls rendered.
- **Issues:** None.

#### Saturation (`saturation`) — OK
- **Expected:** Mode, drive, tone, exciter, oversampling, output gain, mix, dynamic section, toggles.
- **Actual:** Config selectors/toggles + sliders + output knobs.
- **Issues:** None.

#### Band Split (`band_split`) — OK
- **Expected:** Frequency, crossover type (LR24/LR48).
- **Actual:** Knob + two-button set.
- **Issues:** None.

#### Band Merge (`band_merge`) — OK
- **Expected:** Number of bands (2–8).
- **Actual:** Single knob.
- **Issues:** None.

#### Dither (`dither`) — OK
- **Expected:** Bit depth, noise shaping, dither type.
- **Actual:** Toggle + selectors.
- **Issues:** None.

#### Crossover (`crossover`) — No UI
- **Expected:** Plugin crate defines params, but there is no `PluginSettings::Crossover` variant.
- **Actual:** Blank surface.
- **Issues:** `PluginSettings`, `plugin_type_key`, and the custom registry have no `crossover` entry, so the factory-creatable plugin has no GPUI surface.

---

## Cross-cutting findings

1. **Unregistered custom views that look like they should be live but are not:**
   - `ui_gain.rs` (Gain)
   - `ui_downmix.rs` (Downmix)
   - `ui_mono_to_stereo.rs` (Mono to Stereo)
   These files are exported from `components/plugins/mod.rs` but never selected by `render_plugin_content`.

2. **Shared EQ view assumptions break non-standard EQ variants:**
   - `ui_eq/render.rs` assumes a 4-parameter band stride. Linear-Phase EQ and FIR Designer use 5-parameter bands, so their per-band index math is off.

3. **Mode-selector detection is fragile:**
   - `detect_mode_selector` requires an untitled main group with a single `ButtonSet` bound to a `Choice` param. Loudness Compensation uses a `Selector` in the config column and therefore never gets group filtering.

4. **GR/analyzer meters are not universal:**
   - `render_bar_meter` only knows Compressor/Limiter/Gate data. De-Esser publishes its own data type and its meter is dead.

5. **Settings variants without layouts:**
   - `FletcherMunson` and `Crossover` have no editable GPUI surface at all.

---

## Recommendations (summary)

- Register the custom views that are meant to be used, or delete the dead files.
- Fix the EQ shared view so it reads the correct band template and exposes global params for Linear-Phase EQ / FIR Designer.
- Add click handlers to the Channel Mute/Solo M/S/D buttons.
- Add the missing Upmixer params (`speaker_config`, `binaural_preview`, `bandpass_hz`, output auto-gain).
- Add a file picker for XTC `room_ir_file`.
- Wire `DeEsserData` into `render_bar_meter`.
- Provide a `PluginLayout` or custom view for `FletcherMunson` and `Crossover`, or remove them from the available plugin list.

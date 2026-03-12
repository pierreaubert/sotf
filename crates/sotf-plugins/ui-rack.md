# Rack

+----------------------------------------------------------------------+--------------------+
| +-------------------+ .....................                          |                    |
| | A    title       X| .                   .                          |  IN    LABEL  OUT  |
| | S     icon        | .        +          .                          |                    |
| | P                 | .                   .                          |  xx       0    yy  |
| +-------------------+ .....................                          |  xx      -6    yy  |
+----------------------------------------------------------------------+  xx     -12    yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx     -30    yy  |
|                             plugin                                   |  xx            yy  |
|                                                                      |  xx     -60    yy  |
|                                                                      +--------------------+
|                                                                      | Bypass  | AutoGain |
|                                                                      |  Mono   |  M/S     |
|                                                                      |         |          |
+----------------------------------------------------------------------+--------------------+


Description

- each plugin is in the large plugin block
- a box represent each plugin in the rack
- each box has 4 buttons (A is active v.s. bypass, use current icon), S is solo this plugin and mute all the other ones, P is a menu for presets that allows to load and save presets, X allows to remove the plugin (if the plugin is not removable show the locked icon). A, S,P, X are replaced in the real UI by icons.
- We move the input meter to the right box. we have only 1 label column in the middle. On the left we have the input on the right the output. The width of the meter box depends on the number of channel, compute the perfect size and automaticall resize between the plugin and the meter such that the meter are always visible.
- Below the meters, we have 2 rows of buttons. Bypass desactivate the whole chain, autogain is a toggle that activate autogain at the chain level. Mono and M/S are convenience button that do the corresponsing action in the matrix mandatory plugin.

# Upmixer

+----------------------------------------------------------------------------------------+
| UI |                                             | Configuration |  Ouput | Diagnostic |
+----------------------------------------------------------------------------------------+
|
| Channels Gain                        Spatial Control
|
|  Mains Center Surr  Top              Width Spread Bleed Reflect
|
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|
+----------------------------------------------------------------------------------------+
|
| Configuration row
|
+----------------------------------------------------------------------------------------+

# behavior

The menu allow to configure options. Configuration is also a menu but when triggered it fill
the configuration row with what we want to configure (LFE, Dialogue ...)

## UI Menu

- UI
- Simple
- Controller 1
- Controller 2
- ...

## Output menu

the current menu which is 1 row fown

## Diagnostic menu

The 4 on/off options which are currently inside the plugin

- bypass decorrelation
- bypass transient
- bypass all
- bypass ml detection


## Configuration menu

- LFE&Bass
- Dialogue
- Ambient
- Height
- Decorrelation


## Configuration row

### LFE

+----------------------------------------------------------------------------------------+
|
| LFE & Bass          SubHarmonic ON/OFF
|
| LFE Cut Lfe Gain    Gain Freq Attack Release
|
+----------------------------------------------------------------------------------------+


### Dialogue

+----------------------------------------------------------------------------------------+
|
| Dialogue
|
| Weight Voice low Voice high | Center Variance Coherence
|
+----------------------------------------------------------------------------------------+



# Compressor plugin

+--------------------+-------------------------------------------------+--------------------+
| Setup              | Transfer                                        | Meter              |
+--------------------+-------------------------------------------------| Gain reduction
| Link Ch [on|off]   |                                                 |
|                    |  Dynamic               Timing          Transfer +--------------------+
| SC HPF             |  Threshold Ratio Knee  Attack Release  Curve    | Output
|                    |                                                 | AutoGain off/on
+----------------------------------------------------------------------+--------------------+

# Limiter plugin


# Footer

+------------------------------------------------------------------------------+-------------------------+
| pict |    xx:xx                        transport                       yy:yy |   Menu  Menu     Volume |
|      |    -=-====-=--------------------------------------------------------- |   Tool  Devices         |
+------------------------------------------------------------------------------+-------------------------+

# plugin IO logic

I need a plan to redesign most of the UI for plugins that automate the construction of the interface by
setting a set of rules. For each plugin we have a set of parameters which are more or less important and
that we can be group into categories (setup, important, less important, common parameters (meter, autograin, drymix) etc.

Graphically we want a 3 column system. If we have many parameters which are less important we group them in tabs

+--------------------+-------------------------------------------------+--------------------+
| Setup              | Important                                       | Meter              |
+--------------------+-------------------------------------------------| AutoGain           |
| xxx                | yyy                                             | Dry/Mix            |
+----------------------------------------------------------------------+                    |
| tab1 tab2 tab3                                                       |                    |
+----------------------------------------------------------------------+                    |
| less important controls                                              |                    |
+----------------------------------------------------------------------+--------------------+

Do not touch the Rack, EQ, Gain, Matrix, Monitor, XTC plugins.

For each plugins not in the list above:

make 1 to 3 proposals in  ascii form  that shows the layout.

For *all* plugins, verify that all parameters for that plugins are visible in the UI and that they can be changed.


Expected reporting for XTC
in XTC UI,
- Room Reflection toggle does not work AND the title is truncated. (and they are others parameters that do not work)
- Diagnostic could go into less important zone in a tab.
- AG Smoothing is a detail and could also go int the less important zone in another tab
- The overall UI works well.


Plugin UI Redesign Plan

 Context

The current plugin UIs are hand-crafted per-plugin with inconsistent layouts. The goal is to create a rule-based 3-column layout system where plugins declare parameter categories
and importance, and the UI is generated automatically. This reduces per-plugin UI code and ensures consistency.

Excluded plugins (keep as-is): Rack, EQ, Gain, Matrix, Monitor, XTC

---
Phase 1: Layout System Design

3-Column Template

+--------------------+------------------------------------------+--------------------+
| LEFT COLUMN        | CENTER COLUMN                            | RIGHT COLUMN       |
| (Setup/Config)     | (Important / Primary controls)           | (Meter)            |
|                    |                                          | (AutoGain)         |
| Structural params  | Knobs/sliders for main params            | (Dry/Mix)          |
| Mode selectors     |                                          |                    |
| Toggles            +------------------------------------------+                    |
|                    | [Tab1] [Tab2] [Tab3]                     |                    |
|                    +------------------------------------------+                    |
|                    | Less important / advanced controls       |                    |
+--------------------+------------------------------------------+--------------------+

Category System for ParamSpec

Add a category field (or derive from existing group) to classify each param:

┌────────────┬──────────────────────┬───────────────────────────────────────────────────┐
│  Category  │        Column        │                    Description                    │
├────────────┼──────────────────────┼───────────────────────────────────────────────────┤
│ Setup      │ Left                 │ Structural params, mode selectors, channel config │
├────────────┼──────────────────────┼───────────────────────────────────────────────────┤
│ Primary    │ Center-top           │ Main controls users adjust frequently             │
├────────────┼──────────────────────┼───────────────────────────────────────────────────┤
│ Secondary  │ Center-bottom (tabs) │ Advanced/fine-tuning params                       │
├────────────┼──────────────────────┼───────────────────────────────────────────────────┤
│ Output     │ Right                │ Meter, AutoGain, Mix, Makeup                      │
├────────────┼──────────────────────┼───────────────────────────────────────────────────┤
│ Diagnostic │ Center-bottom tab    │ Bypass toggles, debug params                      │
└────────────┴──────────────────────┴───────────────────────────────────────────────────┘

Implementation Approach

New metadata in param_specs.rs:
pub enum ParamCategory {
    Setup,
    Primary,
    Secondary(&'static str), // tab name
    Output,
    Diagnostic,
}

// Add to ParamSpec:
pub category: ParamCategory,

New generic renderer in components/plugins/ui_auto_layout.rs:
- Takes &[ParamSpec], current values, and renders the 3-column layout
- Groups params by category
- Secondary params with different tab names create tabs
- Output column always shows: meter (if available), AutoGain toggle/knobs, Mix knob

Files to Create/Modify

- crates/sotf-plugins/crates/sotf-host/src/param_specs.rs - Add ParamCategory to ParamSpec
- crates/app-gpui/components/plugins/ui_auto_layout.rs - New generic 3-column renderer
- crates/app-gpui/components/plugins/mod.rs - Route plugins to auto layout
- Per-plugin UI files - Replace with category annotations + auto renderer calls

---
Phase 2: Parameter Audit (ALL Plugins)

XTC (excluded from redesign, audit only)

PARAMS (26 params) vs UI (ui_xtc.rs):

┌─────┬──────────────────────────┬──────────────┬───────────────────────────────────────┐
│  #  │          Param           │    In UI?    │                 Issue                 │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 0   │ distance_m               │ Yes (idx 0)  │ OK                                    │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 1   │ speaker_angle_deg        │ Yes (idx 1)  │ OK                                    │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 2   │ head_radius_m            │ Yes (idx 2)  │ OK                                    │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 3   │ head_offset_x            │ Yes (idx 3)  │ OK                                    │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 4   │ head_offset_z            │ Yes (idx 4)  │ OK                                    │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 5   │ head_yaw_deg             │ Yes (idx 5)  │ OK                                    │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 6   │ head_tracking_smooth_s   │ MISSING      │ Not in UI at all                      │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 7   │ beta_base                │ Yes (idx 6)  │ INDEX OFF BY 1 - sends to wrong param │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 8   │ beta_low_freq_boost      │ Yes (idx 7)  │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 9   │ beta_high_freq_boost     │ Yes (idx 8)  │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 10  │ head_shadow_cutoff_hz    │ Yes (idx 9)  │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 11  │ head_shadow_slope        │ Yes (idx 10) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 12  │ max_gain_db              │ Yes (idx 11) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 13  │ spectral_normalization   │ Yes (idx 12) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 14  │ pinna_model_enabled      │ Yes (idx 13) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 15  │ room_reflections_enabled │ Yes (idx 14) │ INDEX OFF BY 1 - toggle broken        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 16  │ room_width_m             │ MISSING      │ Not in UI                             │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 17  │ room_depth_m             │ MISSING      │ Not in UI                             │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 18  │ wall_absorption          │ MISSING      │ Not in UI                             │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 19  │ reflection_beta_boost    │ Yes (idx 18) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 20  │ bypass_xtc_filters       │ Yes (idx 19) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 21  │ bypass_spectral_norm     │ Yes (idx 20) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 22  │ bypass_neumann           │ Yes (idx 21) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 23  │ auto_gain_enabled        │ Yes (idx 22) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 24  │ auto_gain_max_db         │ Yes (idx 23) │ INDEX OFF BY 1                        │
├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤
│ 25  │ auto_gain_smoothing_ms   │ Yes (idx 24) │ INDEX OFF BY 1                        │
└─────┴──────────────────────────┴──────────────┴───────────────────────────────────────┘

XTC Issues:
1. head_tracking_smooth_s (idx 6) missing from UI entirely - causes ALL subsequent param indices to be off by 1
2. room_width_m, room_depth_m, wall_absorption (idx 16-18) missing - Room Reflections toggle exists but room dimension controls are absent
3. "Room Refl" label is truncated (should be "Room Reflections")
4. All params after idx 6 send to wrong param index (off by 1) - Room Reflections toggle and everything else is broken

EQ (excluded, audit only)

- Dynamic filter array - handled by custom EQ editor
- All params accessible via graphical EQ curve + per-band controls
- Status: OK (complex custom UI)

Gain (excluded, audit only)

- 1 param (gain_db) - shown as single knob
- Status: OK

Matrix (excluded, audit only)

- Custom grid UI for channel routing
- Status: OK (custom UI required)

Monitor / LoudnessMonitor (excluded, audit only)

- No user-settable params - analyzer only
- Status: OK

Rack (excluded, audit only)

- Container plugin, not a param plugin
- Status: OK

---
Phase 3: Per-Plugin Layout Proposals (Plugins to Redesign)

Compressor (10 params)

Current UI already uses a nice 3-column layout with transfer curve. Proposal: minor adjustment to match new system.

Proposal A (recommended - close to current):
+------------------+--------------------------------------------+------------------+
| SETUP            | DYNAMICS              TIMING                | OUTPUT           |
|                  |                                            |                  |
| [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |
| [SC HPF]  knob   | [Ratio]     slider    [Release] slider     | [AutoMakeup] tog |
|                  | [Knee]      slider                         | [Makeup]   knob  |
|                  |                                            | [Mix]      knob  |
|                  | ┌─ Transfer Curve ─────────────────┐       |                  |
|                  | │                                  │       |                  |
|                  | └──────────────────────────────────┘       |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 10 params present in current UI. OK.

---
Gate (8 params)

Proposal A (recommended):
+------------------+--------------------------------------------+------------------+
| SETUP            | DYNAMICS              TIMING                | OUTPUT           |
|                  |                                            |                  |
| [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |
| [SC HPF]  knob   | [Ratio]     slider    [Hold]   slider      | [Mix]      knob  |
|                  |                       [Release] slider     |                  |
|                  | ┌─ Transfer Curve ─────────────────┐       |                  |
|                  | │                                  │       |                  |
|                  | └──────────────────────────────────┘       |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 8 params present in current UI. OK.

---
Expander (11 params)

Proposal A (recommended):
+------------------+--------------------------------------------+------------------+
| SETUP            | DYNAMICS              TIMING                | OUTPUT           |
|                  |                                            |                  |
| [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |
| [SC HPF]  knob   | [Ratio]     slider    [Release] slider     | [Mix]      knob  |
|                  | [Range]     slider    [Hold]   slider      |                  |
|                  | [Knee]      slider                         |                  |
|                  | [Hysteresis] slider                        |                  |
|                  | ┌─ Transfer Curve ─────────────────┐       |                  |
|                  | │                                  │       |                  |
|                  | └──────────────────────────────────┘       |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 11 params present in current UI. OK.

---
Limiter (5 params)

Proposal A (recommended - compact):
+------------------+--------------------------------------------+------------------+
| SETUP            | DYNAMICS              TIMING                | OUTPUT           |
|                  |                                            |                  |
| [Soft Knee] tog  | [Threshold] slider    [Release] slider     | [GR Meter]       |
|                  |                       [Lookahead] slider   | [Mix]      knob  |
|                  | ┌─ Limiter Curve ──────────────────┐       |                  |
|                  | │                                  │       |                  |
|                  | └──────────────────────────────────┘       |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 5 params present in current UI. OK.

---
LoudnessCompensation (7 params)

Proposal A (recommended):
+------------------+--------------------------------------------+------------------+
| (empty - no      | LOW                   HIGH                  | AUTO GAIN        |
|  setup params)   |                                            |                  |
|                  | [Low Freq]  knob      [High Freq]  knob    | [AutoGain] tog   |
|                  | [Low Gain]  knob      [High Gain]  knob    | [Max AG]   knob  |
|                  |                                            | [Smoothing] knob |
+------------------+--------------------------------------------+------------------+

Proposal B (2-column, no setup):
+---------------------------------------------------+------------------+
| LOW                       HIGH                      | AUTO GAIN        |
|                                                    |                  |
| [Low Freq] knob           [High Freq] knob         | [AutoGain] tog   |
| [Low Gain] knob           [High Gain] knob         | [Max AG]   knob  |
|                                                    | [Smoothing] knob |
+---------------------------------------------------+------------------+

Audit: All 7 params present in current UI. Need to verify.

---
FletcherMunson (24 params)

Proposal A (recommended - tabs for bands):
+------------------+--------------------------------------------+------------------+
| GLOBAL           | BANDS (4x knob rows)                       | AUTO GAIN        |
|                  |                                            |                  |
| [Playback Vol]   | Band:  [1:Sub]  [2:Bass] [3:Pres] [4:Air] | [AutoGain] tog   |
| [Reference]      | Freq:  [60]     [250]    [3500]   [12000] | [Max Corr] knob  |
| [Enabled] toggle | Q:     [0.5]    [0.707]  [1.0]    [0.707] | [AG Smooth] knob |
| [Smoothing] knob | Max:   [15]     [8]      [4]      [6]     | [AG Loudness]    |
|                  | Slope: [0.6]    [0.4]    [0.2]    [0.3]   |   choice         |
+------------------+--------------------------------------------+------------------+

Proposal B (tabs for less important band details):
+------------------+--------------------------------------------+------------------+
| GLOBAL           | PRIMARY: Band overview                     | AUTO GAIN        |
|                  | Band:  [1:Sub]  [2:Bass] [3:Pres] [4:Air] |                  |
| [Playback Vol]   | Freq:  [60]     [250]    [3500]   [12000] | [AutoGain] tog   |
| [Reference]      | Max:   [15]     [8]      [4]      [6]     | [Max Corr] knob  |
| [Enabled] toggle |                                            | [AG Smooth] knob |
| [Smoothing] knob | [Band Detail] [Auto Gain Detail]           | [AG Loudness]    |
|                  +--------------------------------------------+                  |
|                  | Band Detail tab:                           |                  |
|                  | Q:     [0.5]    [0.707]  [1.0]    [0.707] |                  |
|                  | Slope: [0.6]    [0.4]    [0.2]    [0.3]   |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 24 params. Need to verify current UI covers all.

---
Upmixer (38 params)

Current UI already has tabbed layout. Adapt to 3-column system:

Proposal A (recommended):
+------------------+--------------------------------------------+------------------+
| CONFIG           | GAINS (vertical sliders)                   | OUTPUT           |
|                  |                                            |                  |
| [Speaker Config] | [Front] [FrAmb] [Rear] [Height]           | [Safety Cap]     |
|   choice         | [Width] [CtrSpr] [SurrBleed] [RearAmb]    |                  |
|                  |                                            |                  |
|                  | [LFE&Bass] [Dialogue] [Height] [Decor]    |                  |
|                  +--------------------------------------------+                  |
|                  | Tab: LFE & Bass                            |                  |
|                  | [LFE Gain] [Cutoff] [SubSynth] [SubGain]  |                  |
|                  | [SubFreq] [SubAtk] [SubRel]                |                  |
|                  |                                            |                  |
|                  | Tab: Dialogue                              |                  |
|                  | [Weight] [VoiceMin] [VoiceMax]             |                  |
|                  | [CentroidW] [VarianceW] [CoherenceW]       |                  |
|                  |                                            |                  |
|                  | Tab: Height                                |                  |
|                  | [HF Cap] [Trans Red] [Direct Leak]         |                  |
|                  |                                            |                  |
|                  | Tab: Enhancement                           |                  |
|                  | [HR Direct] [HR Sharpen] [Amb Boost]       |                  |
|                  | [Decor Mode] [LFO Rate] [Velvet Dur/Dens] |                  |
|                  |                                            |                  |
|                  | Tab: Diagnostics                           |                  |
|                  | [Bypass Decor] [Bypass Trans] [Bypass All] |                  |
|                  | [ML Detection]                             |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 38 params covered. Verify: rear_late_reflection (Surround group) must be in spatial sliders or a tab. Upmix Crossover (bandpass_hz) should be in Enhancement or Config
tab.

---
BinauralDecoder (5 params)

Proposal A (compact):
+------------------+--------------------------------------------+------------------+
| SETUP            | CONTROLS                                   | (no meter/AG)    |
|                  |                                            |                  |
| [SOFA File] path | [Externalization] knob                     |                  |
| [Input Ch]  int  | [Near-field]      knob                     |                  |
| [Optim]  toggle  |                                            |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 5 params present in current UI. OK.

---
Convolution (3 params)

Proposal A (minimal):
+------------------+--------------------------------------------+------------------+
| SETUP            | (center empty or waveform display)          | OUTPUT           |
|                  |                                            |                  |
| [IR File]  path  |                                            | [Mix]      knob  |
|                  |                                            | [Gain]     knob  |
+------------------+--------------------------------------------+------------------+

Audit: All 3 params present. OK.

---
SpectrumAnalyzer (6 params)

Proposal A:
+------------------+--------------------------------------------+------------------+
| CONFIG           | SPECTRUM DISPLAY                           | (no output)      |
|                  |                                            |                  |
| [Num Bins]  int  | ┌─ Spectrum Graph ─────────────────┐       |                  |
| [Min Freq]  knob | │                                  │       |                  |
| [Max Freq]  knob | │                                  │       |                  |
| [Smoothing] knob | └──────────────────────────────────┘       |                  |
| [Tilt Corr] choi |                                            |                  |
| [Tilt Ref]  choi |                                            |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 6 params. OK.

---
ChannelMuteSolo (1 param + dynamic channels)

Proposal A:
+------------------+--------------------------------------------+------------------+
| SETUP            | CHANNELS (dynamic per channel count)       | (no output)      |
|                  |                                            |                  |
| [Enabled] toggle | [Ch1: M S D] [Ch2: M S D] [Ch3: M S D].. |                  |
+------------------+--------------------------------------------+------------------+

Audit: OK - dynamic channel states handled by custom code.

---
Denoiser (27 params)

Proposal A (recommended - tabs for advanced):
+------------------+--------------------------------------------+------------------+
| SETUP            | PRIMARY                                    | OUTPUT           |
|                  |                                            |                  |
| [Low Latency]tog | [Reduction] slider   [Floor]   slider     | (no meter)       |
|                  | [Smoothing] knob     [Transparency] knob  |                  |
|                  | [Attack]    knob     [Release] knob        |                  |
|                  |                                            |                  |
|                  | [Analysis] [Hiss] [Spectral] [MCRA] [Prof] |                  |
|                  +--------------------------------------------+                  |
|                  | Tab: Analysis                              |                  |
|                  | [Polyphonic] [Crack Sens] [DD SNR]         |                  |
|                  | [DD Alpha] [Psychoacoustic] [Transient]    |                  |
|                  | [Spectral Smooth] [Temporal Smooth]        |                  |
|                  |                                            |                  |
|                  | Tab: Hiss                                  |                  |
|                  | [Hiss Enable] [Threshold] [Frequency]      |                  |
|                  | [Strength]                                 |                  |
|                  |                                            |                  |
|                  | Tab: Spectral Sub                          |                  |
|                  | [Enabled] [Oversub Factor] [Spectral Floor]|                  |
|                  |                                            |                  |
|                  | Tab: MCRA (Advanced)                       |                  |
|                  | [Alpha S] [Alpha P] [Window] [Delta]       |                  |
|                  |                                            |                  |
|                  | Tab: Noise Profile                         |                  |
|                  | [Learn Noise] [Use Profile] [Clear Profile]|                  |
+------------------+--------------------------------------------+------------------+

Audit: All 27 params covered.

---
Pnd (3 params)

Proposal A (minimal):
+---------------------------------------------------+------------------+
| CONTROLS                                           | (no output)      |
|                                                    |                  |
| [Correction %] knob  [Analysis Window] knob       |                  |
| [Drift Smoothing] knob                             |                  |
+---------------------------------------------------+------------------+

Audit: All 3 params. OK.

---
ABCompare (11 params)

Proposal A:
+------------------+--------------------------------------------+------------------+
| MIX              | PATH CONFIG                                | AUTO GAIN        |
|                  |                                            |                  |
| [Mix A/B]  knob  | [Path A Config] file                       | [AutoGain] tog   |
| [Mix Mode] choic | [Path B Config] file                       | [Loudness] choic |
| [Selected] choic |                                            | [Max AG]   knob  |
| [Bypass]   toggl |                                            | [AG Smooth] knob |
| [Transition] knob|                                            |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 11 params covered.

---
BandSplit (2 params)

Proposal A (minimal):
+---------------------------------------------------+
| [Frequency] knob          [Type: LR24/LR48] choice|
+---------------------------------------------------+

Audit: All 2 params. OK.

---
BandMerge (1 param)

Proposal A (minimal):
+---------------------------------------------------+
| [Bands] integer selector                           |
+---------------------------------------------------+

Audit: 1 param. OK.

---
Downmix (7 params)

Proposal A:
+------------------+--------------------------------------------+------------------+
| PHASE            | GAINS (vertical sliders)                   | (no output)      |
|                  |                                            |                  |
| [Phase Coh] tog  | [Center] [Surround] [Height] [LFE]        |                  |
| [Blend Low] knob |                                            |                  |
| [Blend Hi]  knob |                                            |                  |
+------------------+--------------------------------------------+------------------+

Proposal B (no left column):
+---------------------------------------------------+------------------+
| GAINS                                              | PHASE            |
|                                                    |                  |
| [Center] [Surround] [Height] [LFE] (sliders)      | [Phase Coh] tog  |
|                                                    | [Blend Low] knob |
|                                                    | [Blend Hi]  knob |
+---------------------------------------------------+------------------+

Audit: All 7 params. OK.

---
MonoToStereo (6 params)

Proposal A:
+------------------+--------------------------------------------+------------------+
| SETUP            | CONTROLS                                   | (no output)      |
|                  |                                            |                  |
| [Comp EQ] toggle | [Width]     knob     [Haas Delay] knob    |                  |
| [EQ Depth] knob  | [Decor Low] knob     [Decor High] knob    |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 6 params. OK.

---
Crossfeed (16 params)

Proposal A (recommended - mode-dependent visibility):
+------------------+--------------------------------------------+------------------+
| SETUP            | MODE-SPECIFIC PARAMS                       | AUTO GAIN        |
|                  |                                            |                  |
| [Mode]    choice | If Bauer:                                  | [AutoGain] tog   |
| [Preset]  choice | [Bauer Cutoff] knob  [Bauer Feed] knob    | [Target]   knob  |
| [Enabled] toggle |                                            | [Max Gain] knob  |
| [Mix]     knob   | If Meier:                                  | [Smoothing] knob |
|                  | [Meier Level] knob                         |                  |
|                  |                                            |                  |
|                  | If Multiband:                              |                  |
|                  | [Low Freq] [Mid/Hi Freq]                   |                  |
|                  | [Low Feed] [Mid Feed] [Hi Feed]            |                  |
+------------------+--------------------------------------------+------------------+

Audit: All 16 params. OK.

---
MultibandCompressor (13 global + per-band)

Proposal A:
+------------------+--------------------------------------------+------------------+
| GLOBAL           | BAND VIEW                                  | OUTPUT           |
|                  |                                            |                  |
| [Bands]    int   | [Band 1] [Band 2] [Band 3] ... tabs       | [GR Meters]      |
| [Preset]   int   | Per band:                                  | [Mix]      knob  |
| [Xover 1-4] knob| [Thresh] [Ratio] [Attack] [Release]        | [Link Ch]  tog   |
|                  | [Knee] [Makeup] [Solo] [Bypass]            |                  |
|                  |                                            |                  |
|                  | Global defaults (shown when no band sel):  |                  |
|                  | [Thresh] [Ratio] [Attack] [Release] [Knee] |                  |
+------------------+--------------------------------------------+------------------+

Audit: All global + band params. OK.

---
MultibandExpander (16 global + per-band)

Proposal A (same structure as MB Compressor):
+------------------+--------------------------------------------+------------------+
| GLOBAL           | BAND VIEW                                  | OUTPUT           |
|                  |                                            |                  |
| [Bands]    int   | [Band 1] [Band 2] [Band 3] ... tabs       | [GR Meters]      |
| [Preset]   int   | Per band:                                  | [Mix]      knob  |
| [Xover 1-4] knob| [Thresh] [Ratio] [Attack] [Release]        | [Link Ch]  tog   |
|                  | [Range] [Knee] [Hysteresis] [Hold]         |                  |
|                  | [Solo] [Bypass]                            |                  |
|                  |                                            |                  |
|                  | Global defaults (shown when no band sel):  |                  |
|                  | [Thresh] [Ratio] [Atk] [Rel] [Range]      |                  |
|                  | [Knee] [Hysteresis] [Hold]                 |                  |
+------------------+--------------------------------------------+------------------+

Audit: All global + band params. OK.

---
Phase 4: XTC Bug Fix (Separate from Redesign)

Even though XTC is excluded from redesign, these bugs need fixing:

1. Missing param head_tracking_smooth_s (PARAMS idx 6) causes all subsequent param indices to be off by 1
2. Missing room dimension params (idx 16-18): room_width_m, room_depth_m, wall_absorption
3. "Room Refl" label truncated - should show full "Room Reflections"
4. Fix: Add missing params to XTC render state and UI, fix all param indices

---
Phase 5: Implementation Order

1. Fix XTC param index bug (standalone PR, critical correctness fix)
2. Add ParamCategory to ParamSpec - extend the struct, annotate all plugins
3. Build ui_auto_layout.rs generic renderer using existing common.rs primitives
4. Migrate simple plugins first: Pnd, BandSplit, BandMerge, Convolution, MonoToStereo, LoudnessComp
5. Migrate medium plugins: Gate, Limiter, Expander, Downmix, Binaural, ABCompare
6. Migrate complex plugins: Compressor, Crossfeed, FletcherMunson, Denoiser
7. Migrate tabbed plugins: Upmixer, MultibandCompressor, MultibandExpander
8. Keep custom: SpectrumAnalyzer (graph), ChannelMuteSolo (dynamic channels)

Verification

- cargo check -p sotf-plugins --no-default-features
- cargo check -p app-gpui
- cargo clippy -p sotf-plugins --no-default-features
- cargo test -p sotf-plugins --no-default-features --lib
- Visual: run cargo run --bin sotf_player_tui --release to verify TUI still works
- Visual: run GPUI player to verify each plugin UI renders correctly
- For each plugin: switch to Simple view, count params, compare with PARAMS array length
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
 Let me start by exploring the key files to understand the current codebase structure
 Running 2 Explore agents… (ctrl+o to expand)                                    Plugin UI Redesign Plan

Context

The current plugin UIs are hand-crafted per-plugin with inconsistent layouts. The goal is to create a rule-based 3-column layout system where plugins declare parameter categories
and importance, and the UI is generated automatically. This reduces per-plugin UI code and ensures consistency.
│                                                                                                                                                                                     │
│ Excluded plugins (keep as-is): Rack, EQ, Gain, Matrix, Monitor, XTC                                                                                                                 │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Phase 1: Layout System Design                                                                                                                                                       │
│                                                                                                                                                                                     │
│ 3-Column Template                                                                                                                                                                   │
│                                                                                                                                                                                     │
│ +--------------------+------------------------------------------+--------------------+                                                                                              │
│ | LEFT COLUMN        | CENTER COLUMN                            | RIGHT COLUMN       |                                                                                              │
│ | (Setup/Config)     | (Important / Primary controls)           | (Meter)            |                                                                                              │
│ |                    |                                          | (AutoGain)         |                                                                                              │
│ | Structural params  | Knobs/sliders for main params            | (Dry/Mix)          |                                                                                              │
│ | Mode selectors     |                                          |                    |                                                                                              │
│ | Toggles            +------------------------------------------+                    |                                                                                              │
│ |                    | [Tab1] [Tab2] [Tab3]                     |                    |                                                                                              │
│ |                    +------------------------------------------+                    |                                                                                              │
│ |                    | Less important / advanced controls       |                    |                                                                                              │
│ +--------------------+------------------------------------------+--------------------+                                                                                              │
│                                                                                                                                                                                     │
│ Category System for ParamSpec                                                                                                                                                       │
│                                                                                                                                                                                     │
│ Add a category field (or derive from existing group) to classify each param:                                                                                                        │
│                                                                                                                                                                                     │
│ ┌────────────┬──────────────────────┬───────────────────────────────────────────────────┐                                                                                           │
│ │  Category  │        Column        │                    Description                    │                                                                                           │
│ ├────────────┼──────────────────────┼───────────────────────────────────────────────────┤                                                                                           │
│ │ Setup      │ Left                 │ Structural params, mode selectors, channel config │                                                                                           │
│ ├────────────┼──────────────────────┼───────────────────────────────────────────────────┤                                                                                           │
│ │ Primary    │ Center-top           │ Main controls users adjust frequently             │                                                                                           │
│ ├────────────┼──────────────────────┼───────────────────────────────────────────────────┤                                                                                           │
│ │ Secondary  │ Center-bottom (tabs) │ Advanced/fine-tuning params                       │                                                                                           │
│ ├────────────┼──────────────────────┼───────────────────────────────────────────────────┤                                                                                           │
│ │ Output     │ Right                │ Meter, AutoGain, Mix, Makeup                      │                                                                                           │
│ ├────────────┼──────────────────────┼───────────────────────────────────────────────────┤                                                                                           │
│ │ Diagnostic │ Center-bottom tab    │ Bypass toggles, debug params                      │                                                                                           │
│ └────────────┴──────────────────────┴───────────────────────────────────────────────────┘                                                                                           │
│                                                                                                                                                                                     │
│ Implementation Approach                                                                                                                                                             │
│                                                                                                                                                                                     │
│ New metadata in param_specs.rs:                                                                                                                                                     │
│ pub enum ParamCategory {                                                                                                                                                            │
│     Setup,                                                                                                                                                                          │
│     Primary,                                                                                                                                                                        │
│     Secondary(&'static str), // tab name                                                                                                                                            │
│     Output,                                                                                                                                                                         │
│     Diagnostic,                                                                                                                                                                     │
│ }                                                                                                                                                                                   │
│                                                                                                                                                                                     │
│ // Add to ParamSpec:                                                                                                                                                                │
│ pub category: ParamCategory,                                                                                                                                                        │
│                                                                                                                                                                                     │
│ New generic renderer in components/plugins/ui_auto_layout.rs:                                                                                                                       │
│ - Takes &[ParamSpec], current values, and renders the 3-column layout                                                                                                               │
│ - Groups params by category                                                                                                                                                         │
│ - Secondary params with different tab names create tabs                                                                                                                             │
│ - Output column always shows: meter (if available), AutoGain toggle/knobs, Mix knob                                                                                                 │
│                                                                                                                                                                                     │
│ Files to Create/Modify                                                                                                                                                              │
│                                                                                                                                                                                     │
│ - crates/sotf-plugins/crates/sotf-host/src/param_specs.rs - Add ParamCategory to ParamSpec                                                                                          │
│ - crates/app-gpui/components/plugins/ui_auto_layout.rs - New generic 3-column renderer                                                                                              │
│ - crates/app-gpui/components/plugins/mod.rs - Route plugins to auto layout                                                                                                          │
│ - Per-plugin UI files - Replace with category annotations + auto renderer calls                                                                                                     │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Phase 2: Parameter Audit (ALL Plugins)                                                                                                                                              │
│                                                                                                                                                                                     │
│ XTC (excluded from redesign, audit only)                                                                                                                                            │
│                                                                                                                                                                                     │
│ PARAMS (26 params) vs UI (ui_xtc.rs):                                                                                                                                               │
│                                                                                                                                                                                     │
│ ┌─────┬──────────────────────────┬──────────────┬───────────────────────────────────────┐                                                                                           │
│ │  #  │          Param           │    In UI?    │                 Issue                 │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 0   │ distance_m               │ Yes (idx 0)  │ OK                                    │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 1   │ speaker_angle_deg        │ Yes (idx 1)  │ OK                                    │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 2   │ head_radius_m            │ Yes (idx 2)  │ OK                                    │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 3   │ head_offset_x            │ Yes (idx 3)  │ OK                                    │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 4   │ head_offset_z            │ Yes (idx 4)  │ OK                                    │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 5   │ head_yaw_deg             │ Yes (idx 5)  │ OK                                    │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 6   │ head_tracking_smooth_s   │ MISSING      │ Not in UI at all                      │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 7   │ beta_base                │ Yes (idx 6)  │ INDEX OFF BY 1 - sends to wrong param │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 8   │ beta_low_freq_boost      │ Yes (idx 7)  │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 9   │ beta_high_freq_boost     │ Yes (idx 8)  │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 10  │ head_shadow_cutoff_hz    │ Yes (idx 9)  │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 11  │ head_shadow_slope        │ Yes (idx 10) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 12  │ max_gain_db              │ Yes (idx 11) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 13  │ spectral_normalization   │ Yes (idx 12) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 14  │ pinna_model_enabled      │ Yes (idx 13) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 15  │ room_reflections_enabled │ Yes (idx 14) │ INDEX OFF BY 1 - toggle broken        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 16  │ room_width_m             │ MISSING      │ Not in UI                             │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 17  │ room_depth_m             │ MISSING      │ Not in UI                             │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 18  │ wall_absorption          │ MISSING      │ Not in UI                             │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 19  │ reflection_beta_boost    │ Yes (idx 18) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 20  │ bypass_xtc_filters       │ Yes (idx 19) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 21  │ bypass_spectral_norm     │ Yes (idx 20) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 22  │ bypass_neumann           │ Yes (idx 21) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 23  │ auto_gain_enabled        │ Yes (idx 22) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 24  │ auto_gain_max_db         │ Yes (idx 23) │ INDEX OFF BY 1                        │                                                                                           │
│ ├─────┼──────────────────────────┼──────────────┼───────────────────────────────────────┤                                                                                           │
│ │ 25  │ auto_gain_smoothing_ms   │ Yes (idx 24) │ INDEX OFF BY 1                        │                                                                                           │
│ └─────┴──────────────────────────┴──────────────┴───────────────────────────────────────┘                                                                                           │
│                                                                                                                                                                                     │
│ XTC Issues:                                                                                                                                                                         │
│ 1. head_tracking_smooth_s (idx 6) missing from UI entirely - causes ALL subsequent param indices to be off by 1                                                                     │
│ 2. room_width_m, room_depth_m, wall_absorption (idx 16-18) missing - Room Reflections toggle exists but room dimension controls are absent                                          │
│ 3. "Room Refl" label is truncated (should be "Room Reflections")                                                                                                                    │
│ 4. All params after idx 6 send to wrong param index (off by 1) - Room Reflections toggle and everything else is broken                                                              │
│                                                                                                                                                                                     │
│ EQ (excluded, audit only)                                                                                                                                                           │
│                                                                                                                                                                                     │
│ - Dynamic filter array - handled by custom EQ editor                                                                                                                                │
│ - All params accessible via graphical EQ curve + per-band controls                                                                                                                  │
│ - Status: OK (complex custom UI)                                                                                                                                                    │
│                                                                                                                                                                                     │
│ Gain (excluded, audit only)                                                                                                                                                         │
│                                                                                                                                                                                     │
│ - 1 param (gain_db) - shown as single knob                                                                                                                                          │
│ - Status: OK                                                                                                                                                                        │
│                                                                                                                                                                                     │
│ Matrix (excluded, audit only)                                                                                                                                                       │
│                                                                                                                                                                                     │
│ - Custom grid UI for channel routing                                                                                                                                                │
│ - Status: OK (custom UI required)                                                                                                                                                   │
│                                                                                                                                                                                     │
│ Monitor / LoudnessMonitor (excluded, audit only)                                                                                                                                    │
│                                                                                                                                                                                     │
│ - No user-settable params - analyzer only                                                                                                                                           │
│ - Status: OK                                                                                                                                                                        │
│                                                                                                                                                                                     │
│ Rack (excluded, audit only)                                                                                                                                                         │
│                                                                                                                                                                                     │
│ - Container plugin, not a param plugin                                                                                                                                              │
│ - Status: OK                                                                                                                                                                        │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Phase 3: Per-Plugin Layout Proposals (Plugins to Redesign)                                                                                                                          │
│                                                                                                                                                                                     │
│ Compressor (10 params)                                                                                                                                                              │
│                                                                                                                                                                                     │
│ Current UI already uses a nice 3-column layout with transfer curve. Proposal: minor adjustment to match new system.                                                                 │
│                                                                                                                                                                                     │
│ Proposal A (recommended - close to current):                                                                                                                                        │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | DYNAMICS              TIMING                | OUTPUT           |                                                                                               │
│ |                  |                                            |                  |                                                                                                │
│ | [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |                                                                                                │
│ | [SC HPF]  knob   | [Ratio]     slider    [Release] slider     | [AutoMakeup] tog |                                                                                                │
│ |                  | [Knee]      slider                         | [Makeup]   knob  |                                                                                                │
│ |                  |                                            | [Mix]      knob  |                                                                                                │
│ |                  | ┌─ Transfer Curve ─────────────────┐       |                  |                                                                                                │
│ |                  | │                                  │       |                  |                                                                                                │
│ |                  | └──────────────────────────────────┘       |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 10 params present in current UI. OK.                                                                                                                                     │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Gate (8 params)                                                                                                                                                                     │
│                                                                                                                                                                                     │
│ Proposal A (recommended):                                                                                                                                                           │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | DYNAMICS              TIMING                | OUTPUT           |                                                                                               │
│ |                  |                                            |                  |                                                                                                │
│ | [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |                                                                                                │
│ | [SC HPF]  knob   | [Ratio]     slider    [Hold]   slider      | [Mix]      knob  |                                                                                                │
│ |                  |                       [Release] slider     |                  |                                                                                                │
│ |                  | ┌─ Transfer Curve ─────────────────┐       |                  |                                                                                                │
│ |                  | │                                  │       |                  |                                                                                                │
│ |                  | └──────────────────────────────────┘       |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 8 params present in current UI. OK.                                                                                                                                      │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Expander (11 params)                                                                                                                                                                │
│                                                                                                                                                                                     │
│ Proposal A (recommended):                                                                                                                                                           │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | DYNAMICS              TIMING                | OUTPUT           |                                                                                               │
│ |                  |                                            |                  |                                                                                                │
│ | [Link Ch] toggle | [Threshold] slider    [Attack] slider      | [GR Meter]       |                                                                                                │
│ | [SC HPF]  knob   | [Ratio]     slider    [Release] slider     | [Mix]      knob  |                                                                                                │
│ |                  | [Range]     slider    [Hold]   slider      |                  |                                                                                                │
│ |                  | [Knee]      slider                         |                  |                                                                                                │
│ |                  | [Hysteresis] slider                        |                  |                                                                                                │
│ |                  | ┌─ Transfer Curve ─────────────────┐       |                  |                                                                                                │
│ |                  | │                                  │       |                  |                                                                                                │
│ |                  | └──────────────────────────────────┘       |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 11 params present in current UI. OK.                                                                                                                                     │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Limiter (5 params)                                                                                                                                                                  │
│                                                                                                                                                                                     │
│ Proposal A (recommended - compact):                                                                                                                                                 │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | DYNAMICS              TIMING                | OUTPUT           |                                                                                               │
│ |                  |                                            |                  |                                                                                                │
│ | [Soft Knee] tog  | [Threshold] slider    [Release] slider     | [GR Meter]       |                                                                                                │
│ |                  |                       [Lookahead] slider   | [Mix]      knob  |                                                                                                │
│ |                  | ┌─ Limiter Curve ──────────────────┐       |                  |                                                                                                │
│ |                  | │                                  │       |                  |                                                                                                │
│ |                  | └──────────────────────────────────┘       |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 5 params present in current UI. OK.                                                                                                                                      │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ LoudnessCompensation (7 params)                                                                                                                                                     │
│                                                                                                                                                                                     │
│ Proposal A (recommended):                                                                                                                                                           │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | (empty - no      | LOW                   HIGH                  | AUTO GAIN        |                                                                                               │
│ |  setup params)   |                                            |                  |                                                                                                │
│ |                  | [Low Freq]  knob      [High Freq]  knob    | [AutoGain] tog   |                                                                                                │
│ |                  | [Low Gain]  knob      [High Gain]  knob    | [Max AG]   knob  |                                                                                                │
│ |                  |                                            | [Smoothing] knob |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Proposal B (2-column, no setup):                                                                                                                                                    │
│ +---------------------------------------------------+------------------+                                                                                                            │
│ | LOW                       HIGH                      | AUTO GAIN        |                                                                                                          │
│ |                                                    |                  |                                                                                                           │
│ | [Low Freq] knob           [High Freq] knob         | [AutoGain] tog   |                                                                                                           │
│ | [Low Gain] knob           [High Gain] knob         | [Max AG]   knob  |                                                                                                           │
│ |                                                    | [Smoothing] knob |                                                                                                           │
│ +---------------------------------------------------+------------------+                                                                                                            │
│                                                                                                                                                                                     │
│ Audit: All 7 params present in current UI. Need to verify.                                                                                                                          │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ FletcherMunson (24 params)                                                                                                                                                          │
│                                                                                                                                                                                     │
│ Proposal A (recommended - tabs for bands):                                                                                                                                          │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | GLOBAL           | BANDS (4x knob rows)                       | AUTO GAIN        |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Playback Vol]   | Band:  [1:Sub]  [2:Bass] [3:Pres] [4:Air] | [AutoGain] tog   |                                                                                                 │
│ | [Reference]      | Freq:  [60]     [250]    [3500]   [12000] | [Max Corr] knob  |                                                                                                 │
│ | [Enabled] toggle | Q:     [0.5]    [0.707]  [1.0]    [0.707] | [AG Smooth] knob |                                                                                                 │
│ | [Smoothing] knob | Max:   [15]     [8]      [4]      [6]     | [AG Loudness]    |                                                                                                 │
│ |                  | Slope: [0.6]    [0.4]    [0.2]    [0.3]   |   choice         |                                                                                                 │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Proposal B (tabs for less important band details):                                                                                                                                  │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | GLOBAL           | PRIMARY: Band overview                     | AUTO GAIN        |                                                                                                │
│ |                  | Band:  [1:Sub]  [2:Bass] [3:Pres] [4:Air] |                  |                                                                                                 │
│ | [Playback Vol]   | Freq:  [60]     [250]    [3500]   [12000] | [AutoGain] tog   |                                                                                                 │
│ | [Reference]      | Max:   [15]     [8]      [4]      [6]     | [Max Corr] knob  |                                                                                                 │
│ | [Enabled] toggle |                                            | [AG Smooth] knob |                                                                                                │
│ | [Smoothing] knob | [Band Detail] [Auto Gain Detail]           | [AG Loudness]    |                                                                                                │
│ |                  +--------------------------------------------+                  |                                                                                                │
│ |                  | Band Detail tab:                           |                  |                                                                                                │
│ |                  | Q:     [0.5]    [0.707]  [1.0]    [0.707] |                  |                                                                                                 │
│ |                  | Slope: [0.6]    [0.4]    [0.2]    [0.3]   |                  |                                                                                                 │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 24 params. Need to verify current UI covers all.                                                                                                                         │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Upmixer (38 params)                                                                                                                                                                 │
│                                                                                                                                                                                     │
│ Current UI already has tabbed layout. Adapt to 3-column system:                                                                                                                     │
│                                                                                                                                                                                     │
│ Proposal A (recommended):                                                                                                                                                           │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | CONFIG           | GAINS (vertical sliders)                   | OUTPUT           |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Speaker Config] | [Front] [FrAmb] [Rear] [Height]           | [Safety Cap]     |                                                                                                 │
│ |   choice         | [Width] [CtrSpr] [SurrBleed] [RearAmb]    |                  |                                                                                                 │
│ |                  |                                            |                  |                                                                                                │
│ |                  | [LFE&Bass] [Dialogue] [Height] [Decor]    |                  |                                                                                                 │
│ |                  +--------------------------------------------+                  |                                                                                                │
│ |                  | Tab: LFE & Bass                            |                  |                                                                                                │
│ |                  | [LFE Gain] [Cutoff] [SubSynth] [SubGain]  |                  |                                                                                                 │
│ |                  | [SubFreq] [SubAtk] [SubRel]                |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: Dialogue                              |                  |                                                                                                │
│ |                  | [Weight] [VoiceMin] [VoiceMax]             |                  |                                                                                                │
│ |                  | [CentroidW] [VarianceW] [CoherenceW]       |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: Height                                |                  |                                                                                                │
│ |                  | [HF Cap] [Trans Red] [Direct Leak]         |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: Enhancement                           |                  |                                                                                                │
│ |                  | [HR Direct] [HR Sharpen] [Amb Boost]       |                  |                                                                                                │
│ |                  | [Decor Mode] [LFO Rate] [Velvet Dur/Dens] |                  |                                                                                                 │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: Diagnostics                           |                  |                                                                                                │
│ |                  | [Bypass Decor] [Bypass Trans] [Bypass All] |                  |                                                                                                │
│ |                  | [ML Detection]                             |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 38 params covered. Verify: rear_late_reflection (Surround group) must be in spatial sliders or a tab. Upmix Crossover (bandpass_hz) should be in Enhancement or Config   │
│ tab.                                                                                                                                                                                │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ BinauralDecoder (5 params)                                                                                                                                                          │
│                                                                                                                                                                                     │
│ Proposal A (compact):                                                                                                                                                               │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | CONTROLS                                   | (no meter/AG)    |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [SOFA File] path | [Externalization] knob                     |                  |                                                                                                │
│ | [Input Ch]  int  | [Near-field]      knob                     |                  |                                                                                                │
│ | [Optim]  toggle  |                                            |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 5 params present in current UI. OK.                                                                                                                                      │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Convolution (3 params)                                                                                                                                                              │
│                                                                                                                                                                                     │
│ Proposal A (minimal):                                                                                                                                                               │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | (center empty or waveform display)          | OUTPUT           |                                                                                               │
│ |                  |                                            |                  |                                                                                                │
│ | [IR File]  path  |                                            | [Mix]      knob  |                                                                                                │
│ |                  |                                            | [Gain]     knob  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 3 params present. OK.                                                                                                                                                    │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ SpectrumAnalyzer (6 params)                                                                                                                                                         │
│                                                                                                                                                                                     │
│ Proposal A:                                                                                                                                                                         │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | CONFIG           | SPECTRUM DISPLAY                           | (no output)      |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Num Bins]  int  | ┌─ Spectrum Graph ─────────────────┐       |                  |                                                                                                │
│ | [Min Freq]  knob | │                                  │       |                  |                                                                                                │
│ | [Max Freq]  knob | │                                  │       |                  |                                                                                                │
│ | [Smoothing] knob | └──────────────────────────────────┘       |                  |                                                                                                │
│ | [Tilt Corr] choi |                                            |                  |                                                                                                │
│ | [Tilt Ref]  choi |                                            |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 6 params. OK.                                                                                                                                                            │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ ChannelMuteSolo (1 param + dynamic channels)                                                                                                                                        │
│                                                                                                                                                                                     │
│ Proposal A:                                                                                                                                                                         │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | CHANNELS (dynamic per channel count)       | (no output)      |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Enabled] toggle | [Ch1: M S D] [Ch2: M S D] [Ch3: M S D].. |                  |                                                                                                  │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: OK - dynamic channel states handled by custom code.                                                                                                                          │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Denoiser (27 params)                                                                                                                                                                │
│                                                                                                                                                                                     │
│ Proposal A (recommended - tabs for advanced):                                                                                                                                       │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | PRIMARY                                    | OUTPUT           |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Low Latency]tog | [Reduction] slider   [Floor]   slider     | (no meter)       |                                                                                                 │
│ |                  | [Smoothing] knob     [Transparency] knob  |                  |                                                                                                 │
│ |                  | [Attack]    knob     [Release] knob        |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | [Analysis] [Hiss] [Spectral] [MCRA] [Prof] |                  |                                                                                                │
│ |                  +--------------------------------------------+                  |                                                                                                │
│ |                  | Tab: Analysis                              |                  |                                                                                                │
│ |                  | [Polyphonic] [Crack Sens] [DD SNR]         |                  |                                                                                                │
│ |                  | [DD Alpha] [Psychoacoustic] [Transient]    |                  |                                                                                                │
│ |                  | [Spectral Smooth] [Temporal Smooth]        |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: Hiss                                  |                  |                                                                                                │
│ |                  | [Hiss Enable] [Threshold] [Frequency]      |                  |                                                                                                │
│ |                  | [Strength]                                 |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: Spectral Sub                          |                  |                                                                                                │
│ |                  | [Enabled] [Oversub Factor] [Spectral Floor]|                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: MCRA (Advanced)                       |                  |                                                                                                │
│ |                  | [Alpha S] [Alpha P] [Window] [Delta]       |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Tab: Noise Profile                         |                  |                                                                                                │
│ |                  | [Learn Noise] [Use Profile] [Clear Profile]|                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 27 params covered.                                                                                                                                                       │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Pnd (3 params)                                                                                                                                                                      │
│                                                                                                                                                                                     │
│ Proposal A (minimal):                                                                                                                                                               │
│ +---------------------------------------------------+------------------+                                                                                                            │
│ | CONTROLS                                           | (no output)      |                                                                                                           │
│ |                                                    |                  |                                                                                                           │
│ | [Correction %] knob  [Analysis Window] knob       |                  |                                                                                                            │
│ | [Drift Smoothing] knob                             |                  |                                                                                                           │
│ +---------------------------------------------------+------------------+                                                                                                            │
│                                                                                                                                                                                     │
│ Audit: All 3 params. OK.                                                                                                                                                            │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ ABCompare (11 params)                                                                                                                                                               │
│                                                                                                                                                                                     │
│ Proposal A:                                                                                                                                                                         │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | MIX              | PATH CONFIG                                | AUTO GAIN        |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Mix A/B]  knob  | [Path A Config] file                       | [AutoGain] tog   |                                                                                                │
│ | [Mix Mode] choic | [Path B Config] file                       | [Loudness] choic |                                                                                                │
│ | [Selected] choic |                                            | [Max AG]   knob  |                                                                                                │
│ | [Bypass]   toggl |                                            | [AG Smooth] knob |                                                                                                │
│ | [Transition] knob|                                            |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 11 params covered.                                                                                                                                                       │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ BandSplit (2 params)                                                                                                                                                                │
│                                                                                                                                                                                     │
│ Proposal A (minimal):                                                                                                                                                               │
│ +---------------------------------------------------+                                                                                                                               │
│ | [Frequency] knob          [Type: LR24/LR48] choice|                                                                                                                               │
│ +---------------------------------------------------+                                                                                                                               │
│                                                                                                                                                                                     │
│ Audit: All 2 params. OK.                                                                                                                                                            │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ BandMerge (1 param)                                                                                                                                                                 │
│                                                                                                                                                                                     │
│ Proposal A (minimal):                                                                                                                                                               │
│ +---------------------------------------------------+                                                                                                                               │
│ | [Bands] integer selector                           |                                                                                                                              │
│ +---------------------------------------------------+                                                                                                                               │
│                                                                                                                                                                                     │
│ Audit: 1 param. OK.                                                                                                                                                                 │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Downmix (7 params)                                                                                                                                                                  │
│                                                                                                                                                                                     │
│ Proposal A:                                                                                                                                                                         │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | PHASE            | GAINS (vertical sliders)                   | (no output)      |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Phase Coh] tog  | [Center] [Surround] [Height] [LFE]        |                  |                                                                                                 │
│ | [Blend Low] knob |                                            |                  |                                                                                                │
│ | [Blend Hi]  knob |                                            |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Proposal B (no left column):                                                                                                                                                        │
│ +---------------------------------------------------+------------------+                                                                                                            │
│ | GAINS                                              | PHASE            |                                                                                                           │
│ |                                                    |                  |                                                                                                           │
│ | [Center] [Surround] [Height] [LFE] (sliders)      | [Phase Coh] tog  |                                                                                                            │
│ |                                                    | [Blend Low] knob |                                                                                                           │
│ |                                                    | [Blend Hi]  knob |                                                                                                           │
│ +---------------------------------------------------+------------------+                                                                                                            │
│                                                                                                                                                                                     │
│ Audit: All 7 params. OK.                                                                                                                                                            │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ MonoToStereo (6 params)                                                                                                                                                             │
│                                                                                                                                                                                     │
│ Proposal A:                                                                                                                                                                         │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | CONTROLS                                   | (no output)      |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Comp EQ] toggle | [Width]     knob     [Haas Delay] knob    |                  |                                                                                                 │
│ | [EQ Depth] knob  | [Decor Low] knob     [Decor High] knob    |                  |                                                                                                 │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 6 params. OK.                                                                                                                                                            │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Crossfeed (16 params)                                                                                                                                                               │
│                                                                                                                                                                                     │
│ Proposal A (recommended - mode-dependent visibility):                                                                                                                               │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | SETUP            | MODE-SPECIFIC PARAMS                       | AUTO GAIN        |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Mode]    choice | If Bauer:                                  | [AutoGain] tog   |                                                                                                │
│ | [Preset]  choice | [Bauer Cutoff] knob  [Bauer Feed] knob    | [Target]   knob  |                                                                                                 │
│ | [Enabled] toggle |                                            | [Max Gain] knob  |                                                                                                │
│ | [Mix]     knob   | If Meier:                                  | [Smoothing] knob |                                                                                                │
│ |                  | [Meier Level] knob                         |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | If Multiband:                              |                  |                                                                                                │
│ |                  | [Low Freq] [Mid/Hi Freq]                   |                  |                                                                                                │
│ |                  | [Low Feed] [Mid Feed] [Hi Feed]            |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All 16 params. OK.                                                                                                                                                           │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ MultibandCompressor (13 global + per-band)                                                                                                                                          │
│                                                                                                                                                                                     │
│ Proposal A:                                                                                                                                                                         │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | GLOBAL           | BAND VIEW                                  | OUTPUT           |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Bands]    int   | [Band 1] [Band 2] [Band 3] ... tabs       | [GR Meters]      |                                                                                                 │
│ | [Preset]   int   | Per band:                                  | [Mix]      knob  |                                                                                                │
│ | [Xover 1-4] knob| [Thresh] [Ratio] [Attack] [Release]        | [Link Ch]  tog   |                                                                                                 │
│ |                  | [Knee] [Makeup] [Solo] [Bypass]            |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Global defaults (shown when no band sel):  |                  |                                                                                                │
│ |                  | [Thresh] [Ratio] [Attack] [Release] [Knee] |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All global + band params. OK.                                                                                                                                                │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ MultibandExpander (16 global + per-band)                                                                                                                                            │
│                                                                                                                                                                                     │
│ Proposal A (same structure as MB Compressor):                                                                                                                                       │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│ | GLOBAL           | BAND VIEW                                  | OUTPUT           |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ | [Bands]    int   | [Band 1] [Band 2] [Band 3] ... tabs       | [GR Meters]      |                                                                                                 │
│ | [Preset]   int   | Per band:                                  | [Mix]      knob  |                                                                                                │
│ | [Xover 1-4] knob| [Thresh] [Ratio] [Attack] [Release]        | [Link Ch]  tog   |                                                                                                 │
│ |                  | [Range] [Knee] [Hysteresis] [Hold]         |                  |                                                                                                │
│ |                  | [Solo] [Bypass]                            |                  |                                                                                                │
│ |                  |                                            |                  |                                                                                                │
│ |                  | Global defaults (shown when no band sel):  |                  |                                                                                                │
│ |                  | [Thresh] [Ratio] [Atk] [Rel] [Range]      |                  |                                                                                                 │
│ |                  | [Knee] [Hysteresis] [Hold]                 |                  |                                                                                                │
│ +------------------+--------------------------------------------+------------------+                                                                                                │
│                                                                                                                                                                                     │
│ Audit: All global + band params. OK.                                                                                                                                                │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Phase 4: XTC Bug Fix (Separate from Redesign)                                                                                                                                       │
│                                                                                                                                                                                     │
│ Even though XTC is excluded from redesign, these bugs need fixing:                                                                                                                  │
│                                                                                                                                                                                     │
│ 1. Missing param head_tracking_smooth_s (PARAMS idx 6) causes all subsequent param indices to be off by 1                                                                           │
│ 2. Missing room dimension params (idx 16-18): room_width_m, room_depth_m, wall_absorption                                                                                           │
│ 3. "Room Refl" label truncated - should show full "Room Reflections"                                                                                                                │
│ 4. Fix: Add missing params to XTC render state and UI, fix all param indices                                                                                                        │
│                                                                                                                                                                                     │
│ ---                                                                                                                                                                                 │
│ Phase 5: Implementation Order                                                                                                                                                       │
│                                                                                                                                                                                     │
│ 1. Fix XTC param index bug (standalone PR, critical correctness fix)                                                                                                                │
│ 2. Add ParamCategory to ParamSpec - extend the struct, annotate all plugins                                                                                                         │
│ 3. Build ui_auto_layout.rs generic renderer using existing common.rs primitives                                                                                                     │
│ 4. Migrate simple plugins first: Pnd, BandSplit, BandMerge, Convolution, MonoToStereo, LoudnessComp                                                                                 │
│ 5. Migrate medium plugins: Gate, Limiter, Expander, Downmix, Binaural, ABCompare                                                                                                    │
│ 6. Migrate complex plugins: Compressor, Crossfeed, FletcherMunson, Denoiser                                                                                                         │
│ 7. Migrate tabbed plugins: Upmixer, MultibandCompressor, MultibandExpander                                                                                                          │
│ 8. Keep custom: SpectrumAnalyzer (graph), ChannelMuteSolo (dynamic channels)                                                                                                        │
│                                                                                                                                                                                     │
│ Verification                                                                                                                                                                        │
│                                                                                                                                                                                     │
│ - cargo check -p sotf-plugins --no-default-features                                                                                                                                 │
│ - cargo check -p app-gpui                                                                                                                                                           │
│ - cargo clippy -p sotf-plugins --no-default-features                                                                                                                                │
│ - cargo test -p sotf-plugins --no-default-features --lib                                                                                                                            │
│ - Visual: run cargo run --bin sotf_player_tui --release to verify TUI still works                                                                                                   │
│ - Visual: run GPUI player to verify each plugin UI renders correctly                                                                                                                │
│ - For each plugin: switch to Simple view, count params, compare with PARAMS array length                                                                                            │
╰─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯


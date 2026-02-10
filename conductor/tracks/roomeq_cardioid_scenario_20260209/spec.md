# Track: RoomEQ Cardioid Scenario

## 1. Overview
Rename `scenario_03_small_multi_sub` to `scenario_03_small_stereo_2_2_mso`.
Add `scenario_03_small_stereo_2_2_cardioid` with stacked subs.
Update `roomeq_config_gen` to produce `CardioidConfig` for the new scenario.

## 2. Changes

### 2.1 Scenarios (`crates/autoeq-datagen/src/scenarios.rs`)
*   Rename `scenario_03_small_multi_sub` -> `scenario_03_small_stereo_2_2_mso`.
    *   Change name string to `"small_stereo_2_2_mso"`.
*   Add `scenario_03_small_stereo_2_2_cardioid`.
    *   Mains same.
    *   Subs: `sub_front` (bottom) and `sub_rear` (top). Or `sub_main`, `sub_cardioid`.
    *   I'll use `sub_bottom` (Z=0.15) and `sub_top` (Z=0.65). Separation 0.5m.
    *   Name string: `"small_stereo_2_2_cardioid"`.

### 2.2 Config Gen (`crates/autoeq-datagen/src/roomeq_config_gen.rs`)
*   Update detection logic:
    *   If scenario source names contain `sub_bottom` and `sub_top`, create `SpeakerConfig::Cardioid`.
    *   Map `front` -> `sub_bottom`, `rear` -> `sub_top`.
    *   Set `separation_meters` = 0.5.
    *   Else if multiple subs -> `SpeakerConfig::MultiSub`.

## 3. Verification
*   Compile.
*   (Optional) Generate data and check JSON.

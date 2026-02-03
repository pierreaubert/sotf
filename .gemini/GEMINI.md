Refactor `find_device_by_name` logic in `sotf-audio-engine`

- Improved audio device matching logic in `sotf-audio-engine/src/signal_recorder.rs` and `sotf-audio-engine/src/engine/playback_thread.rs`.
- The matching now prioritizes:
    1. Exact match (case-insensitive)
    2. "Starts with" match (case-insensitive) - **New**
    3. "Contains" match (case-insensitive)
- This fixes an issue where selecting a device (e.g., "Microphone") could incorrectly match a different device (e.g., "Built-in Microphone") if it appeared earlier in the list, instead of the intended target (e.g., "Microphone (USB)").
- This resolves the reported issues:
    - **Recording:** Ensures the correct microphone is selected even if names are similar.
    - **Upmixer/Playback:** Ensures the correct multi-channel output device is selected, preventing fallback to default stereo devices which caused 5-channel upmixing to be downmixed or fail.

- Refactored `ABCompare` UI in `sotf-audio-player` to match the strict ASCII art grid layout.
- Implemented a 2-column layout with explicit horizontal and vertical borders (`border_b_1`, `border_r_1`).
- Used `items_stretch` to ensure the vertical separator extends fully.
- Replaced internal padding/gaps with `p_4` wrappers for each cell to create distinct grid areas.
- Aligned controls:
    - Left Col: Mode | Mix & Path | Path Configs
    - Right Col: Gain Auto | Gain & Time | Smoothing Horizontal Sliders

- Fixed audio crackling/distortion in HAL input:
    - Added `available_read_frames` method to `HalInputReader` in `driver-hal`.
    - Updated `DecoderThread` in `engine` to:
        1. Check if sufficient frames are available in the shared memory ring buffer before reading, preventing partial reads/silence gaps.
        2. Automatically resample HAL input (using `ResamplerPlugin`) if the HAL sample rate differs from the engine's target sample rate (e.g., 44.1kHz vs 48kHz). This prevents pitch shifts and buffer underruns caused by rate mismatches.


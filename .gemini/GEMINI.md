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

# Product Guidelines - SotF (Sound of the Future)

## Tone and Voice
- **Technical and Precise:** Documentation and user-facing messages must prioritize accuracy, mathematical detail, and engineering rigor. 
- Use standard industry terminology (e.g., Q-factor, shelving filter, FFT size) to ensure clarity for professional users and audiophiles.
- Avoid ambiguous or overly marketing-heavy language; focus on the data and the functional reality of the audio processing.

## Design Principles
- **High Information Density:** The UI (especially in the GPUI and TUI) should prioritize exposing relevant data and controls. Don't hide complexity if it provides value to the user's optimization workflow.
- Visual layouts should be organized to show relationships between parameters (e.g., linking a slider to its corresponding point on a frequency response plot).
- Ensure that visual feedback, such as spectrum analyzers or level meters, remains fluid and accurate without compromising the real-time audio thread performance.

## User Interaction (Acoustic Optimization)
- **Wizard-Guided Workflows:** While the underlying engine is powerful, the primary user interface for complex tasks (like AutoEQ or Room Correction) should be guided by "Wizards."
- Break down complex optimizations into clear, sequential steps (e.g., Select Input -> Choose Target -> Run Optimization -> Review Results).
- Provide sensible defaults for each step to ensure a high success rate for non-experts, while allowing experts to "eject" from the wizard to manually tweak parameters.

## Platform Integration & Consistency
- **Platform-Native Experience:** Strive for a "first-class citizen" feel on every operating system by leveraging platform-specific UI conventions and integration points (e.g., macOS menu bars, system notifications).
- Ensure that OS-specific audio backends (CoreAudio/HAL on macOS, ALSA/Pipewire on Linux, WASAPI on Windows) are utilized optimally to provide the lowest possible latency.

## Error Handling & Reliability
- **Rich Diagnostic Feedback:** Implement comprehensive logging across the audio engine and plugin system. In case of failure, provide users with actionable technical details or log files that can be used for debugging.
- Use the TUI and GUI to display clear status indicators for background operations (e.g., "Calculating Filters", "Resampling HAL Input") to manage user expectations.

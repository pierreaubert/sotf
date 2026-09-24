# RoomEQ multi-microphone capture protocol

This protocol targets two to four independently clocked USB measurement
microphones. Post-correction does not make stacked UMIKs sample-synchronous
hardware. Existing single-microphone recording and REW `.mdat`, `.msop`, and WAV
imports remain available.

Implementation status: CLI, TUI, and GPUI share plan validation, simultaneous
raw acquisition, offline clock correction, calibrated analysis, and canonical
RoomEQ import. AutoEQ reports expose clock QA and conditional compact-array
directions; the RIR prototype can use accepted measured directions. Synthetic
fixtures verify processing and fallback behavior. Physical multi-device capture
and cross-platform device operation still require manual validation. See
[issue #173](http://192.168.1.32:3001/pierre/sotf/issues/173).
Plan validation does not open devices, read calibration files, or certify takes.

## Declare the session

Start with the [two-microphone example](../crates/sotf-player/tests/fixtures/capture-session.json).
Replace the example device names, calibration paths, positions, gains, and output
routes with those of the measurement setup. Validate it with:

```sh
cargo run -p app-cli --bin roomeq-capture -- validate session.json
```

The plan uses schema version 1. Device selectors must be explicit. Prefer stable
IDs shown by `sotf-recorder-cli --list-devices`; unique device names also work.
Ambiguous names are rejected. Input and output
channel numbers are zero-based, with supported routes 0–63. Each microphone has a unique identity and
device/channel route; each source has a unique identity and output channel.
Sources are ordered for sequential excitation, with every microphone recording
each source. Capture resolves calibration paths relative to the plan file and
preserves calibration snapshots and their SHA-256 identities before opening audio.

Acquire raw takes into a new directory with:

```sh
cargo run -p app-cli --bin roomeq-capture -- \
  record session.json --output-dir capture-001
```

The parent directory must exist and the destination must not exist. Ctrl-C stops
acquisition. Completed source recordings and the `capture-raw.json` journal remain
available after cancellation or a later device failure. The journal retains the
frozen plan, calibration snapshots, actual device IDs and sample formats, stimulus
layout, raw WAV paths, peak/clipping counts, and explicit acquisition status.
`raw_complete` means only acquisition finished: clock correction, calibrated
analysis and take-quality decisions remain pending. Raw WAVs are device-clock
recordings with independent origins. Do not use them as synchronized pair sums.
The journal is not yet a `recordings.json` measurement import.
The existing recorder also exposes the same operations through
`--validate-capture-session session.json` and
`--capture-session session.json --output-dir capture-001`.

## Fixed acoustic timing reference

Both chirps use the same loudspeaker for every source take. By default this is
the first source's output channel. For geometric clock correction, declare that
emitter's surveyed acoustic center in the microphone coordinate frame:

```json
"timing_reference": {
  "output_channel": 0,
  "position_m": [2.0, 0.0, 1.2],
  "position_uncertainty_mm": 1.0,
  "sound_speed_m_s": 343.0,
  "sound_speed_uncertainty_m_s": 0.2
}
```

Replace these example values with the actual setup and uncertainty estimates.
An acoustic marker includes propagation delay: independently shifting every
microphone's direct arrival to zero would erase the delays needed for direction
finding. The processor subtracts the surveyed propagation contribution from the
clock-offset estimate, preserving physical arrival differences. Without a survey,
arrival alignment remains available for magnitude processing, but its clock
residual is unknown and coherent use is disabled. Older raw journals without a
fixed timing route are also kept magnitude-only.

## Offline clock processing

```sh
cargo run -p app-cli --bin roomeq-capture -- \
  process capture-001 --output-dir capture-001-clock
```

The destination must be new. Processing verifies calibration hashes, journal
routes, stimulus layout, and raw WAV rate/format/length. It writes per-take audio
and `capture-clock.json`, retaining device IDs, offset, skew, correction applied,
the timing-reference identity, uncertainty, and magnitude-only reasons. A bad or
missing end chirp preserves the original samples in the processed output instead
of discarding the take. Unknown uncertainty is JSON `null`, never a numeric zero.

The clock gate requires a finite nonnegative residual below **50 µs**, and coherent
consumers must also match the fixed timing-reference identity. This represents
nine degrees at 500 Hz; higher-frequency and array consumers need additional
phase/aperture limits. Bounds include the chirp model's three-sigma envelope,
sampling/interpolation allowance, chirp dilation, microphone/emitter survey error,
and sound-speed uncertainty. They assume linear clock drift and isolated,
correctly identified timing chirps. They do not certify arbitrary multipath or
nonlinear drift. A passing clock fit alone does not certify calibration or overall take QA;
those are evaluated separately in the analysis stage.

The measurement resampler uses a 129-tap Kaiser FIR with interpolation between
1024 phase-table entries. Its tested band is 0–20 kHz at 48 kHz (scaled with sample
rate); frequencies near Nyquist are filtered to prevent aliasing during clock
correction. Raw files are retained for future reprocessing.

Choose one geometry for the entire session:

- `spread`: microphones occupy the seats to be compared. Retain individual seat
  magnitude responses. Do not infer reflection directions from widely separated
  seats.
- `compact`: microphones form a centimeter-scale cluster with surveyed positions
  accurate to one millimeter. Record coordinates in meters in one common frame.
  Two microphones can exercise timing correction; direction estimation requires
  at least three and suitable geometry. A planar array cannot resolve every
  elevation ambiguity. Aperture, frequency, calibration, and timing uncertainty
  still constrain usable directions even after the plan passes validation.

`calibration_orientation` is `on_axis` or `ninety_degrees`. Mount each microphone
in the orientation matching its individual calibration file. `gain_db` records
the input gain; it does not command an OS mixer or apply a software gain. Set and
document gains before starting, and keep them fixed for all sources and takes.

The sweep declares duration in seconds, start/end frequencies in Hz, and linear
peak amplitude in `(0, 1]`. Both sweep limits must be below Nyquist. Nominal sample
rate must be in 16001–384000 Hz to accommodate the 2–8 kHz timing chirp and
the supported capture limits. Duration is bounded by the per-microphone storage
budget; excessively long sweeps fail before waveform allocation. Every selected
device must support the same actual nominal sample rate; never silently fall
back to a different rate or device.

## Physical setup and acquisition

Use stationary stands and mark every position. Stop HVAC and other avoidable
noise, keep people still, and avoid touching USB cables or microphone stands
during a take. Start at a conservative playback level. Check input peak headroom
and the quiet-room noise floor before the sweep; a louder sweep is not a remedy
for clipping, rattles, or unstable gain control.

For each source, the capture pipeline must record all microphone streams across
pre-silence, a start timing chirp, the logarithmic sweep and decay tail, an end
timing chirp, and post-silence. Timing chirps must be acoustically audible on every
microphone. Reserve quiet gaps so sweep decay does not obscure the end chirp.
Do not reuse one microphone's timing evidence for another device.

Protocol v1 emits 500 ms silence, a 100 ms 2–8 kHz timing chirp, a 500 ms gap,
the declared sweep, two seconds of decay silence, the repeated timing chirp, and
500 ms final silence. Chirps and sweeps use 5 ms edge tapers. The engine also
captures at least 250 ms before playback and 500 ms after submitting the last
output buffer. Those extra input samples are not assumed to align across devices.
A long room decay can obscure the end chirp; timing analysis must flag that take
instead of accepting an unreliable fit.

Inspect every take for clipping, dropouts, noise, chirp confidence, and residual
timing uncertainty. Preserve raw recordings alongside correction parameters and
corrected outputs. Missing or degraded chirps make timing unavailable; they do
not imply zero drift or zero uncertainty. Such recordings remain candidates for
magnitude-only use, subject to their separate clipping and SNR checks.

## Operating-system setup

- **macOS:** identify devices in Audio MIDI Setup and set compatible nominal
  rates. If using an Aggregate Device, document its clock source, channel order,
  and whether OS drift correction is enabled. OS drift correction is not proof
  of the residual acoustic timing bound. Prevent automatic gain processing.
- **Linux:** identify ALSA/PipeWire devices and disable voice-processing,
  automatic gain, and unintended resampling where supported. Document routing
  and any virtual aggregate. A common PipeWire graph rate does not establish a
  shared physical USB clock.
- **Windows:** document WASAPI devices and channel assignments, disable audio
  enhancements and automatic gain, and select matching formats. Windows does
  not provide a universal native aggregate-device setup for unrelated USB mics;
  use independently opened supported inputs or document the installed aggregate
  driver. Do not assume exclusive access or a driver shared clock.

## Required take QA and viewer behavior

Completed capture output must persist device identity, per-microphone calibration
identity, session geometry, offset in samples, skew in ppm, correction applied,
and residual uncertainty in microseconds. These take fields are not yet produced
by the plan-validation command.

Coherent pair sums and direction overlays must remain unavailable until every
participating microphone passes the documented residual timing threshold.
Missing, non-finite, or excessive bounds require a visible magnitude-only reason;
pending analysis must never appear as success. Compact-mode measured directions
must also pass the array geometry/frequency checks before replacing assumed
reflection geometry in reports or directivity models.


## Offline calibrated magnitude and quality evidence

`roomeq-capture process RAW_DIRECTORY --output-dir NEW_DIRECTORY` preserves
raw takes and writes clock-corrected audio plus `capture-clock.json`. Each take's
`analysis` records an optional calibrated magnitude CSV, original clipped-sample
count, broadband SNR, octave-band SNR, and review issues. Calibration subtracts that microphone's
frozen response deviation; CSV frequencies stay inside both the sweep and
calibration bounds. Values retain relative digital gain and are not calibrated
absolute SPL. No coherent phase is implied by this magnitude-only CSV.

Broadband SNR compares sweep power against the middle of the pre-chirp quiet
interval, subtracting noise power. Below 30 dB raises a review issue. Missing
windows, zero digital noise, or signal below noise leave SNR unavailable.
This metric does not replace frequency-dependent SNR for decay or reflection
analysis. Clipping is assessed on original samples, before resampling can hide it.
A missing clock fit preserves original audio and attempts magnitude-only analysis
using the measurement sweep's correlation lock. This uses the original device
clock, never supplies phase or a timing bound, and warns that drift can smear the
magnitude response. Without a reliable sweep lock or enough samples to exclude
the end timing chirp, no magnitude artifact is emitted. Broadband SNR remains
unavailable for these uncorrected takes. Sweep analysis requires its own confident
correlation lock, independently of timing-chirp fit.

When every requested source/microphone pair has a calibrated magnitude export,
processing also writes the canonical `recordings.json`. It groups microphone
measurements by source, preserves microphone order, and includes
`provenance.capture.takes` with clock, calibration, gain, orientation, and position
facts. Failed timing fits remain present with null bounds. This manifest supports
magnitude EQ. Per-take `quality_passed` flags become true only when every
microphone for that source has accepted shared-reference phase artifacts and
passes clipping, broadband SNR, and frequency-band SNR checks. Incomplete
analysis keeps the export pending rather than silently dropping microphones.

`capture-clock.json` retains detailed raw-artifact references and review issues.
Reflection reports retain accepted candidates and explicit refusal reasons.

The AutoEQ HTML viewer now shows **Capture clock QA** when these capture facts
are present in the result's effective configuration. Its L+R comparison checks
every microphone's bound, correction, quality, reference, and geometry before
using measured phase. Missing or failed evidence produces a labeled
magnitude-only power sum with the reason shown. No phase or group-delay overlay
is derived from that fallback. Magnitude-only exports remain
unavailable for coherent analysis.

### Frequency-dependent capture SNR

The analysis `frequency_snr` array records lower/upper band edges and optional
SNR in dB. Octave-width bands cover the sweep range. Corrected takes compare
the sweep window with the middle pre-chirp quiet interval using periodic-Hann
Welch spectra, 50% overlap, identical FFT sizes, and window-energy PSD
normalization. Unequal window durations therefore do not change the power ratio.
The FFT size is the largest power of two within half the shorter window, capped
at 4096 samples; no zero padding is used. Bands without usable bins or positive
noise-subtracted signal remain unknown. Digital silence never implies infinite
SNR. Values below 30 dB raise a review issue.

This is time-averaged sweep-window SNR, assuming stationary background noise;
it is not instantaneous sweep SNR or deconvolved reflection SNR. It does not
authorize reflection detection on its own. Uncorrected takes and legacy reports
have no frequency-band evidence and cannot pass coherent acceptance. The
shared-origin artifacts below require accepted frequency-band evidence.

### Shared-origin phase and impulse artifacts

A take with a surveyed fixed timing reference, no timing refusal, and passed
clipping and SNR checks can additionally contain `analysis.common_reference`.
Its `response_file` is a calibrated frequency/magnitude/phase CSV; its
`impulse_file` is a real-valued IR JSON file. The sweep and recording use the
same window origin. No correlation lag is removed and no impulse peak is
rotated. Each microphone therefore retains its physical propagation delay.

Magnitude calibration scales complex bins by a positive real gain. It does
not correct unknown microphone phase. Bins outside the sweep/calibration
intersection are zeroed for IR reconstruction, making the IR band-limited and
periodic over its FFT length. These artifacts are not absolute-SPL measurements.
The IR metadata records this interpretation explicitly.

`timing_limit_hz` is the upper frequency for nine degrees of conditional clock
phase error, calculated as 25000 divided by the residual bound in microseconds
and limited to the calibrated sweep range. Measured phase above this limit is
not permission for coherent combination. Canonical `recordings.json` exports these phase CSVs only when every microphone
for a source has accepted phase artifacts and SNR evidence. Otherwise that
source retains magnitude CSVs. Coherent operation boundaries additionally check
the requested upper frequency against every microphone clock bound. The viewer
checks worst-case relative L/R timing error over the plotted frequency range.

### Compact-array arrival candidates

`capture-clock.json` now includes `reflection_reports` keyed by source. The
existing SSIR detector operates on the first microphone shared-origin IR to
identify a direct-arrival candidate and early-reflection candidates. Up to 64
early events within 80 ms of the direct candidate are retained. These are
energy-based candidates, not independently surveyed reflection labels.

Each event window is analyzed across all microphones with C1 pair-delay
estimates and the C5 least-squares array solver. The analysis band is the
intersection of the calibrated sweep, 300–3000 Hz, and the worst-case relative
clock bound for nine degrees of phase error. Windows need at least two periods
of the lowest analysis frequency. Unreliable pair delays, delays exceeding the
array aperture, and plane-wave fit residuals above half a sample are refused.
The surveyed sound speed is retained by scaling the C5 geometry consistently.

Two-microphone and collinear geometries cannot yield a unique direction. Planar
arrays retain their mirror ambiguity, including tilted planes. Accepted vectors
point toward the source or image source, in the session coordinates. They remain
conditional on a plane wave and matched microphone phase; source distance and
phase are not independently verified. Fit residuals are not angular-error bounds.
The CLI reports candidate counts and missing/ambiguous direction reasons.
Canonical provenance preserves these records under
`provenance.capture.reflection_report`. AutoEQ HTML reports render per-channel
arrival tables and conditional direction/delay plots. Invalid clock evidence,
unsupported timing bandwidth, and planar mirror ambiguity suppress plotted
directions. RIR-prototype weighting can use validated measured directions and event energies as described below.

### Measured-direction RIR prototype weights

Each arrival records `microphone_energy_db`: integrated SSIR-segment energy
relative to that microphones direct segment, in capture order. When
`multi_measurement.rir_prototype` is enabled, RoomEQ carries this evidence
through its prepared EQ resources. The prototype forms an energy-weighted
mixture of directivity factors per microphone. Measured arrival angles replace
geometric angles only in their supported frequency band. The head-forward
axis points from `reference_position` toward `source_position`.

This is a magnitude-domain spatial-weight approximation. It does not reconstruct
coherent reflection interference or synthesize microphone phase. Missing event
energies, failed clock/quality evidence, geometry mismatch, planar ambiguity, or
insufficient geometric height relative to survey uncertainty preserve geometric
weights. Unsupported individual events and frequency bins also retain the
geometric model. The engine logs measured-direction bin counts per microphone.

### Frontend import and export

In GPUI, open **Recording → Multi-mic capture**. Enter the session-plan JSON,
raw directory, and processed directory paths. **Load and review plan** shows
the frozen geometry, microphone calibrations, routes, and gains; it does not
open audio devices. **Record all microphones** starts acquisition, and
**Cancel recording** preserves completed raw takes. **Process raw session**
also accepts sessions recorded earlier by the CLI or TUI. Read the QA section
before selecting **Import into RoomEQ**. Import stays disabled until a complete
canonical manifest exists. **Back to recording** becomes available when idle.
Inputs and buttons support Tab navigation and keyboard activation. Processing
and recording continue if another app screen is selected; return to this panel
to inspect progress or cancel recording.

In the TUI, open **Configure → Recording** and press **F8** for multi-mic capture.
Use **Tab/Up/Down** to choose a path field and **Enter** to edit it; **Enter** or
**Esc** ends editing. Enter the session-plan JSON path, a new raw directory, and
a new processed directory. The raw directory's parent must already exist.

Press **V** to validate and review the plan. Check geometry, device/channel
routes, every microphone's calibration and orientation, positions, and frozen
gains. Validation does not open hardware or certify take quality. Press **R**
to record every source on all microphones, or **C** to request cancellation.
Completed raw takes remain on disk. Input edits and other recording workflows
are blocked while the worker is active.

Press **P** to process the raw session, including a previously saved session.
Processing runs in the background and must finish before another operation.
Use **PageUp/PageDown** to inspect drift bounds, clipping, SNR bands, phase
availability, reflection ambiguities, and pending stages. **I** imports into
RoomEQ only when a complete canonical manifest exists; inspect QA before using
it. Raw completion alone never enables import. **Esc/F8** returns to the regular
recording wizard when idle. Processing a saved session disarms the loaded plan;
load and review it again before starting a new recording.

RoomEQ retains declared acquisition evidence when importing canonical manifests,
saving frontend state, and exporting a RoomConfig. Each driver in a grouped
speaker retains its own evidence. Legacy files without acquisition metadata
remain unknown, and importing a failed clock bound never upgrades it.

Evidence is bound to the ordered frequency, magnitude, phase, and sample-rate
data loaded by the frontend. Editing, removing, or reordering responses revokes
quality acceptance on export. Raw clock and calibration facts remain available
for inspection, but cannot authorize coherent processing. These response hashes
detect frontend changes; they are not authentication of externally supplied files.

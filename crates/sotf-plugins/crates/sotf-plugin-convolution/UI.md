# Convolution UI contract

The authoritative layout is `params::LAYOUT`.

- Main `CONVOLUTION` group: IR file picker, Mix, and Gain.
- Advanced tab: Use NUPC, Zero-Latency Head, and Head Taps.
- Use NUPC, Zero-Latency Head, and Head Taps are structural; changing them requires plugin rebuild.
- IR loading is asynchronous. UI integrations should show `ConvolutionLoadStatus` as
  idle/loading/ready/failed and keep displaying the last active path until a replacement succeeds.
- Mix is a normalized 0–1 dry/wet value. Gain is -20 to +20 dB. Head Taps is 32–512.

The backend keeps dry audio latency-aligned during empty/loading/failed/cleared states, so a UI or
host bypass must not substitute an undelayed parallel path for the plugin output.

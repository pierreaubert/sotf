# SOTF testing system

SOTF uses separate test tiers so deterministic correctness tests stay fast while
hardware, stress, and performance checks retain their own budgets.

## Local commands

```text
just test-unit-core          # shared helpers, engine, player, plugin units
just test-integration-engine # decoder and manager integration contracts
just test-integration-player
just test-device-fakes       # deterministic device-selection/orchestration tests
just test-realtime-safety    # allocation and realtime-path checks
just test-pr                 # all deterministic PR tiers
just coverage-core           # per-core-crate coverage reports
just perf-smoke              # release DAW realtime/deadline smoke gate
just test-nightly            # full nextest/property/coverage/performance tier
```

Tests requiring a real virtual or physical audio device must not be added to
`test-unit-core` or `test-device-fakes`. They belong in the engine QA matrix,
the systemwide lab, or portability CI and must skip with an explicit reason
when the backend is unavailable.

## Required test contracts

Decoder tests cover format detection, malformed/truncated input, EOF, seeking,
metadata, variable frame sizes, reusable destinations, and finite/aligned
output. Processing tests cover block sizes, channel layouts, sample rates,
bypass/reset, latency, parameter boundaries, non-finite input, and allocation
behavior. Manager tests cover command ordering, restart/fatal paths, gapless
transitions, event delivery, and idempotent shutdown. Device tests use fakes for
enumeration, selection, fallback, negotiation, disconnect, and reconnect;
platform smoke tests cover only the actual backend boundary.

Use `sotf-testkit::assertions` for shared audio invariants rather than repeating
slightly different finite/range/frame-alignment checks in each crate.

## Coverage policy

`coverage-core` is the baseline-reporting command. Coverage must be tracked per
core crate and ratcheted from the recorded baseline; workspace-wide coverage is
still useful for reporting but should not hide untested decoder/manager/device
code behind generated, UI, or platform-only code.

The intended steady-state targets are:

- 90% line / 80% branch coverage for pure decoder and DSP logic;
- 80% line / 70% branch coverage for manager and device orchestration;
- every public manager state transition reached by at least one test;
- no allocation or deadline regressions in realtime processing paths.

Targets should be introduced from a checked-in baseline and raised rather than
made retroactively mandatory in one change.

## Performance policy

`perf-smoke` runs the release DAW stress harness and applies the existing
absolute realtime/deadline budget. For historical comparisons, use
`just perf-regression baseline.csv candidate.csv [tolerance]`. Candidate runs
must use the same sample rate, block size, chain, track count, and runner class.

Performance gates compare p99 realtime factor and deadline misses. A p99
regression is allowed only within the configured relative tolerance; any new
deadline miss fails the gate. Noisy hardware-sensitive benchmarks belong in
nightly or release CI, not in every developer edit loop.

## CI tiers

- Pull requests: formatting, changed-crate checks, unit tests, deterministic
  integration tests, fake-device tests, and realtime allocation checks.
- Nightly: full nextest, high-case property tests, coverage, stress/fuzz tests,
  and the complete performance matrix.
- Portability/release: macOS, Linux, and Windows backend/plugin smoke tests,
  including real-device checks where required.

The call-graph report is a prioritization metric. It complements LLVM line and
branch coverage and must not be treated as runtime coverage by itself.

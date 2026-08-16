# AAE acoustic-quality validation

The deterministic gate is:

```bash
cargo test -p sotf-plugin-aae --locked
cargo run --release -p sotf-plugin-aae --features qa --bin qa-aae-quality --locked \
  | tee /tmp/aae-quality.tsv
```

It renders a representative matrix spanning Small–Cathedral rooms, 5.1 and
9.1.6, 44.1/48 kHz, and 64/257/1024-frame partitions. The program reports
bandwise Schroeder T20-derived RT60, normalized echo density/mixing time,
inter-channel coherence, energy entropy/vector/diffuseness, end-to-end LFE
magnitude/phase, modulation sidebands, THD/IMD, exact linked-limiter gain,
synthetic detector precision/recall, and detector gain variation.

## External validation (not satisfied by synthetic tests)

Listening quality and corpus generalization remain explicitly external. Do not
replace them with generated signals or claim that the deterministic gate proves
them. For a release candidate, use the same level-matched input files and record:

```text
fixture_id,label,license_or_source,sha256,layout,sample_rate,expected_dialogue_regions
```

The fixture set must contain at least: clean and noisy dialogue; off-centre and
hard-panned speech; mono/stereo music; percussion; anti-phase material; diffuse
ambience; sustained bass; and transient-rich full-scale programme. Store only
redistributable material in the repository. For restricted material, store the
manifest/hash and acquisition instructions outside version control.

Run randomized, double-blind, loudness-matched A/B comparisons against bypass
and the previous release for every room preset and both 5.1 and 9.1.6. Collect
envelopment, timbral neutrality, dialogue clarity, pumping, bass localization,
and preference ratings, plus detector region TP/FP/FN counts. Archive the exact
binary commit, parameter JSON, device/room, listener count, anonymized ratings,
and `/tmp/aae-quality.tsv`. No listening/corpus acceptance claim is made until
those artifacts exist.

## Machine-checkable manifest and report

The repository provides a verifier for the external artifacts. The structural
schemas are [quality-validation-manifest.schema.json](quality-validation-manifest.schema.json)
and [quality-validation-run.schema.json](quality-validation-run.schema.json).
The manifest is JSON with `schema_version: 1` and a `fixtures` array. Each fixture contains
`fixture_id`, `label`, `license_or_source`, lowercase `sha256`, `layout`,
`sample_rate`, `expected_dialogue_regions`, and an optional `path`. The verifier
requires all eleven fixture classes named above, both `5.1` and `9.1.6`, unique
IDs, valid dialogue ranges, and matching hashes when a path is supplied. Restricted
fixtures can omit `path`; their hash and acquisition source remain required.

The listening run JSON records `binary_commit`, `parameter_json`,
`device_and_room`, `listener_count`, `qa_output_sha256`, the manifest digest,
both bypass/previous-release comparisons, per-condition ratings, and detector
TP/FP/FN counts. The default acceptance thresholds are at least 8 listeners and
8 ratings per fixture/layout condition, detector precision and recall of 0.80,
mean preference of 4/7, and mean pumping no worse than 3/7.

Validate and emit the report with:

```bash
cargo run -p sotf-plugin-aae --bin qa-aae-validation -- \
  --manifest validation-manifest.json \
  --run validation-run.json \
  --report validation-report.json \
  --fixture-root /path/to/corpus
```

Without `--run`, the report is deliberately `accepted: false` and marks
`external_evidence: false`; deterministic QA evidence is never promoted to an
external acceptance claim. Keep licensed/restricted corpus files and run JSON
outside version control, retaining only redistributable fixtures and manifests
where licensing permits.

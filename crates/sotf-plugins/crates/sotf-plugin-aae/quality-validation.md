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

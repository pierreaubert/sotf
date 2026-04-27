---
title: autoeq-download-speakers
description: Download speaker measurements from spinorama.org.
---

Downloads speaker frequency response measurements from the [spinorama.org](https://api.spinorama.org) API.
Measurements are saved as CSV files for use with `autoeq` or `roomeq`.

## Synopsis

```
autoeq-download-speakers [OPTIONS]
```

## Flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-f` / `--force` | bool | `false` | Re-download measurements that already exist locally |
| `-s` / `--speaker` | string | — | Filter by speaker name (case-insensitive substring match) |

## Examples

Download all measurements:
```bash
autoeq-download-speakers
```

Download only measurements matching "kef":
```bash
autoeq-download-speakers --speaker kef
```

Force re-download of all KEF measurements:
```bash
autoeq-download-speakers --speaker kef --force
```

## API Source

Measurements come from `https://api.spinorama.org`. The tool fetches all speaker names,
then downloads measurement versions and CSV data for each. Measurements are saved to
the current directory.

Use the downloaded files as `--curve` input for `autoeq` or in `roomeq` config `speakers` paths.

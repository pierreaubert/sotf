---
title: convert-recording
description: Convert or rewrite RoomEQ files to the current schema versions.
---

Converts measurement files saved by older versions of the SotF desktop app
(`recording.json`) to the current `roomeq` input format (`RoomConfig` JSON).
It also rewrites existing `RoomConfig` input files and `DspChainOutput`
output files using the latest schema version.

## Synopsis

```
convert-recording <INPUT> [OUTPUT]
```

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `INPUT` | Yes | Path to a legacy `recording.json`, `RoomConfig`, or `DspChainOutput` file |
| `OUTPUT` | No | Output path. Defaults to overwriting `INPUT` (with `.bak` backup) |

## Examples

Convert in-place (creates `recording.json.bak`):
```bash
convert-recording recording.json
```

Convert to a new file:
```bash
convert-recording recording.json room-config.json
```

Rewrite an existing file in-place to the latest schema version:
```bash
convert-recording room-config.json
```

## When to Use

Use this tool when you have `recording.json` files created by SotF versions
before 0.5.x, or when you want to normalize existing RoomEQ input/output JSON
after a schema version bump.

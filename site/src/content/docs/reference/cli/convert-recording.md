---
title: convert-recording
description: Convert legacy recording.json files to the current RoomConfig format.
---

Converts measurement files saved by older versions of the SotF desktop app
(`recording.json`) to the current `roomeq` input format (`RoomConfig` JSON).

## Synopsis

```
convert-recording <INPUT> [OUTPUT]
```

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `INPUT` | Yes | Path to legacy `recording.json` file |
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

## When to Use

If you have `recording.json` files created by SotF versions before 0.5.x,
they use a legacy measurement format that the current `roomeq` CLI does not
accept directly. Run `convert-recording` once and then use the output file
as your `roomeq --config` input.

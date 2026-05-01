---
title: Queue Screen
description: View and manage the current playback queue.
---

The Queue screen shows the list of tracks scheduled for playback. You can reorder, remove,
and inspect tracks from this screen.

## Layout

The queue displays tracks in playback order. The currently playing track is highlighted.
Progress through the queue advances automatically when each track ends.

## Navigation

| Key (Terminal) | Action |
|-----------|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `Enter` | Jump to selected track and play |
| `d` | Remove selected track from queue |
| `c` | Clear the entire queue |
| `J` | Move selected track down (reorder) |
| `K` | Move selected track up (reorder) |

## Adding Tracks

Tracks are added to the queue from the **Library** screen:
- Press `Enter` on an album to replace the queue and start playing
- Press `a` to append the album to the end of the queue without interrupting playback

## Queue Modes

The queue respects the **repeat** and **shuffle** settings configured on the playback bar:
- **Shuffle** — randomizes playback order (does not reorder the visual list)
- **Repeat All** — loops the entire queue when the last track ends
- **Repeat One** — loops the current track indefinitely

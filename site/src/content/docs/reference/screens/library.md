---
title: Library Screen
description: Browse, search, and manage your music collection.
---

The Library screen displays your indexed music collection organized by artist and album.
It is the primary navigation point for finding and playing music.

## Layout

The screen is split into three panels:

- **Left** — Artist/album list (filtered by current search or sort)
- **Center** — Track list for the selected album
- **Right** — Album art and metadata (if available)

## Navigation

| Key (TUI) | Action |
|-----------|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `Enter` | Play selected album / add track to queue |
| `a` | Add to queue without playing |
| `/` | Start search |
| `Esc` | Clear search |
| `Tab` | Switch between panels |

## Searching

Press `/` to open the search bar. The library filters in real time as you type.
Search matches artist name, album title, and track title.

Press `Esc` to clear the search and show all albums.

## Sorting

The library can be sorted by:
- Artist (default)
- Album title
- Year
- Recently added

Switch sort mode from the **Configure** screen or with the keybinding shown in the help overlay.

## Library Scanning

Add music directories in **Configure → Directories**. After adding a new directory,
trigger a scan to index new files. Existing entries are updated incrementally on subsequent
scans — only changed files are re-read.

Metadata is read from embedded tags (ID3, FLAC, Vorbis Comment). Files without tags
still appear but may show "Unknown Artist" / "Unknown Album".

## Supported Formats

FLAC, MP3, AAC, ALAC (M4A), OGG Vorbis, WAV.

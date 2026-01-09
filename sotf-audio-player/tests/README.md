# Integration Tests for sotf-audio-player

This directory contains comprehensive integration tests for the `sotf-audio-player` crate.

## Test Coverage

### Database Tests (`database_tests.rs`)
Tests for the SQLite database functionality:
- ✅ **Database Creation**: Verify database file creation and WAL mode activation.
- ✅ **Save and Load**: Test album and track persistence.
- ✅ **Normalized Metadata**: Verify storage in junction tables (genres, composers, etc.).
- ✅ **Search Functionality**: FTS5 full-text search integration.
- ✅ **Clean Missing Files**: Remove tracks for deleted files.
- ✅ **ReplayGain Storage**: Store and retrieve ReplayGain values.

### Library Tests (`library_tests.rs`)
Tests for music library management:
- ✅ **Library Creation**: Initialize library with database.
- ✅ **Directory Management**: Add/remove watched directories.
- ✅ **File Scanning**: Scan directories for audio files.
- ✅ **Metadata Extraction**: Extract duration, channels, etc.
- ✅ **Library Persistence**: Save and reload library state.
- ✅ **Incremental Scanning**: Skip unchanged files on rescan.

### ReplayGain Tests (`replay_gain_tests.rs`)
Tests for ReplayGain scanning functionality:
- ✅ **Scanner Creation**: Initialize ReplayGainScanner.
- ✅ **Update ReplayGain**: Store gain and peak values.
- ✅ **Value Persistence**: ReplayGain survives database reloads.

### Album Art Tests (`album_art_tests.rs`) 🆕
- ✅ **Discovery**: Find `cover.png`, `folder.jpg`, etc., in music directories.
- ✅ **Subdirectory Search**: Support for `Artwork/` and `Covers/` folders.
- ✅ **Thumbnail Generation**: Automatic PNG thumbnail creation (160x160).
- ✅ **Persistence**: Thumbnails are stored as BLOBs in the database.

### Concurrency Tests (`concurrent_db_tests.rs`) 🆕
- ✅ **Stress Test**: Multiple writers and readers accessing the DB simultaneously.
- ✅ **Deadlock Prevention**: Uses **WAL mode** and **IMMEDIATE transactions** to ensure thread safety.
- ✅ **Busy Timeout**: 5-second timeout to handle transient locks gracefully.

### Error Handling Tests (`error_handling_tests.rs`) 🆕
- ✅ **Corrupted Files**: Gracefully skip non-audio files with music extensions.
- ✅ **Mixed Sets**: Ensure valid files are still indexed even if corrupted files exist in the same folder.

## Database Performance & Reliability
The `MusicDatabase` has been hardened for concurrent use:
1. **WAL Mode**: Enabled for better read/write concurrency.
2. **Busy Timeout**: Set to 5 seconds to prevent "database is locked" errors.
3. **Immediate Transactions**: All writes use `TransactionBehavior::Immediate` to prevent deadlocks.

## Test Infrastructure
The `fixtures.rs` module provides shared utilities:
- `demo_audio_dir()` - Path to demo audio files (`assets/demo-audio/`).
- `temp_database()` - Create temporary test database.
- `copy_demo_files_to_temp()` - Copy files for isolated testing.

## Running Tests
Tests require the `testing` feature to bypass security path validation:
```bash
cargo test -p sotf-audio-player --features testing
```

# [0.8.2] - 2026-07-08

## Added
- QA-SEC-006 negative abuse tests for malformed command frames, invalid UTF-8 lines, and missing/wrong password authentication.

# [0.5.1] - 2025-05-13

## Added
- Initial release of MPD protocol server for SOTF.
- TCP server accepting MPD protocol connections (`MpdServer`).
- Command parsing and response formatting (`MpdCommand`, `MpdResponse`).
- Optional TLS support behind the `tls` feature.

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.8.2 [Unreleased]

### Added
- Added client-certificate generation/load helpers for mobile and desktop
  pairing flows.
- Added a persistent trusted-client certificate store for server-side mTLS
  pairing and revocation.

## [0.8.1] - 2025-05-13

### Added
- Initial release of TLS infrastructure for SOTF.
- Self-signed certificate generation via `rcgen`.
- Trust-On-First-Use (TOFU) persistent trust store.
- TLS client and server config builders.
- Mutual TLS support.

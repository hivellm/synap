# Changelog

All notable changes to the Synap Go SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-07-11

### Added
- First tagged release of the Go SDK: KV, Hash, List, Set, Sorted Set, Queue,
  Stream and Pub/Sub modules over three transports (SynapRPC default, RESP3,
  HTTP), selected by URL scheme.

### Changed
- Version aligned with the Synap server 1.0.0 release. SynapRPC (`synap://host:15501`) is the default transport; RESP3 and HTTP remain available via URL scheme. Test suite verified against the official `hivehub/synap:1.0.0` image.
- `stretchr/testify` 1.6 → 1.11.

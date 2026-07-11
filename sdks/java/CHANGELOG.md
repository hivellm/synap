# Changelog

All notable changes to the Synap Java SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-07-11

### Added
- First tagged release of the Java SDK: KV, Hash, List, Set, Queue, Stream and
  Pub/Sub managers over three transports (SynapRPC default, RESP3, HTTP),
  selected by URL scheme.

### Changed
- Version aligned with the Synap server 1.0.0 release. SynapRPC (`synap://host:15501`) is the default transport; RESP3 and HTTP remain available via URL scheme. Test suite verified against the official `hivehub/synap:1.0.0` image.
- Jackson 2.17.1 → 2.18.2.

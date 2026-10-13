# Proposal: phase27_gui-server-contracts

Source: docs/analysis/next-release-review-2026-10-12/ (findings E-02, priority P1, and E-03, priority P2)
Target release: 1.4.0

## Why

- **E-02**: the configuration view (`gui/src/views/ConfigView.vue`) loads with
  the `admin.config` command, which no server dispatcher defines, so loading
  always fails and the view silently shows a built-in sample as if it were the
  server's configuration. `saveConfig` never sends anything to the server. It
  updates local state and shows "Configuration saved successfully!".
  Operators believe they changed durability or security settings when nothing
  changed.
- **E-03**: the structure inspector (`gui/src/services/api.ts`) sends `kv.type`
  and `kv.memory`, but the server registers `key.type` and `memory.usage`
  (`server/handlers/mod.rs`). The type lookup fails and the inspector
  (`StructuresView.vue`) clears itself for every hash, list, set and sorted set.

## What Changes

- The server gets an admin-only command that returns the effective
  configuration with secrets redacted (passwords, API keys, Hub keys).
- The configuration view loads from it. A failed load shows an error and never
  the sample.
- The view becomes read-only, with copy and download of the YAML. Remote
  configuration writes are deliberately not added: they would open a
  privileged write surface, and most settings need a restart anyway. The fake
  save path is removed.
- The inspector calls `key.type` and `memory.usage` and parses their real
  responses.
- GUI API client contract tests run against a real server.

## Impact

- Affected specs: gui (configuration view, structure inspector), server
  (config read command)
- Affected code: `gui/src/views/ConfigView.vue`, `gui/src/services/api.ts`,
  `gui/src/views/StructuresView.vue`,
  `crates/synap-server/src/server/handlers/admin_cmd.rs`
- Dependencies: phase16 (admin authorization of the new command)
- Breaking change: NO. The editor never applied anything.
- User benefit: the GUI shows the server's real configuration and real
  structure contents.

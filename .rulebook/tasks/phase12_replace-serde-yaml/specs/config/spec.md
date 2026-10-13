## MODIFIED Requirements

### Requirement: Maintained configuration parser
The server and the migration tool MUST parse YAML configuration through a
maintained crate, and `deny.toml` MUST NOT ignore RUSTSEC-2024-0449 once the
swap lands.

#### Scenario: Shipped configs parse identically after the swap
Given each file under `config/` parsed into `ServerConfig` with `serde_yaml` 0.9
When the same file is parsed with the replacement crate
Then the resulting `ServerConfig` is equal field for field

#### Scenario: Advisory gate without the libyaml ignore
Given RUSTSEC-2024-0449 is removed from the `deny.toml` ignore list
When `cargo deny check advisories` runs
Then it passes because `unsafe-libyaml` is no longer in the graph

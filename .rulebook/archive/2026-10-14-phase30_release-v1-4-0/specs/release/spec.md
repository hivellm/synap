# Release 1.4.0

## ADDED Requirements

### Requirement: Aligned versions
Every published artifact MUST carry version 1.4.0.

#### Scenario: Version check
Given the release branch
When the versions of the Cargo workspace, every SDK and the GUI are read
Then each one is 1.4.0

### Requirement: Upgrade from 1.3.2
Synap 1.4.0 MUST recover a data directory written by 1.3.2 without losing
acknowledged data.

#### Scenario: Upgrade in place
Given a data directory written by 1.3.2 with a snapshot and a WAL tail
When 1.4.0 starts on it
Then every key, structure and queue message present before the upgrade is readable

### Requirement: Documented migration
The release MUST ship a migration guide covering every behavior change and
stating that downgrading after 1.4.0 has written data is unsupported.

#### Scenario: Operator reads the guide
Given the 1.4.0 migration guide
When an operator plans the upgrade
Then the guide lists the backup step, the no-downgrade rule and every changed default

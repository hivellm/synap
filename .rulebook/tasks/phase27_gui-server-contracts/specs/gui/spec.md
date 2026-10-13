# GUI Server Contracts

## ADDED Requirements

### Requirement: Real configuration, read-only
The server MUST expose its effective configuration to admin users with secrets
redacted, and the GUI MUST display that configuration read-only and MUST NOT
display sample data as the server's configuration.

#### Scenario: Admin views configuration
Given a server with `auth.users` containing a password and an admin GUI session
When the configuration view opens
Then it shows the server's effective configuration
And every password and key value is redacted

#### Scenario: Load fails
Given a GUI session without admin rights
When the configuration view opens
Then it shows a permission error
And no configuration content is displayed

#### Scenario: Non-admin request
Given a read-only user
When the user calls the configuration command directly
Then the server denies it

### Requirement: Inspector uses server commands
The structure inspector MUST use the command names and response shapes the
server defines.

#### Scenario: Inspect a hash
Given hash `h` with two fields created through the server
When the user selects `h` in the inspector
Then the inspector shows type `hash`, both fields and the memory usage

#### Scenario: Missing key
Given no key `ghost`
When the user inspects `ghost`
Then the inspector reports that the key does not exist

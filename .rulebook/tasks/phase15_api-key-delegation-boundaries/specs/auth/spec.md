# API Key Issuance

## ADDED Requirements

### Requirement: Authenticated issuer
When authentication is enabled, the server MUST reject API key creation from a
request that carries no authenticated identity.

#### Scenario: Anonymous creation rejected
Given authentication is enabled with `require_auth: false`
When an unauthenticated client posts a valid body to `/auth/keys`
Then the server answers 401
And no API key is stored

### Requirement: Bounded delegation
The server MUST create an API key only when the issuer's effective permissions
cover every requested grant, where a grant is covered when every resource its
pattern can match is matched by an issuer permission whose action includes the
requested action. Admin users SHALL be allowed to grant any permission.

#### Scenario: Read-only user cannot mint a wildcard key
Given a user whose only role is read-only
When the user requests a key with resource `*` and action `all`
Then the server answers 403
And no API key is stored

#### Scenario: Resource-scoped user cannot widen the pattern
Given a user granted `write` on `kv:orders:*`
When the user requests a key with `write` on `kv:*`
Then the server answers 403

#### Scenario: Narrower grant accepted
Given a user granted `write` on `kv:orders:*`
When the user requests a key with `read` on `kv:orders:eu:*`
Then the key is created
And a request authenticated with it can read `kv:orders:eu:1` and cannot write it

#### Scenario: Restricted key cannot mint a broader successor
Given an API key granted `read` on `kv:*` that expires in one hour
When a request authenticated with that key asks for `write` on `kv:*`, or for a key without expiry
Then the server answers 403

### Requirement: Strict request validation
The server MUST reject an API key request containing an unknown permission
action or an unparsable `allowed_ips` entry, instead of defaulting the action or
dropping the address.

#### Scenario: Unknown action
Given an admin user
When the user requests a grant with action `wrte`
Then the server answers 400 naming the action

#### Scenario: Invalid IP
Given an admin user
When the user requests a key with `allowed_ips: ["10.0.0.300"]`
Then the server answers 400 naming the address
And no API key is stored

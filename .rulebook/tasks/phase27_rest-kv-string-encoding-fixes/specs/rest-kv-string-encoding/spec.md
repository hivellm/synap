# REST KV string encoding

## ADDED Requirements

### Requirement: APPEND encodes strings the way SET does
The REST `POST /kv/{key}/append` handler SHALL encode a JSON string value as
raw UTF-8, matching `POST /kv/set`, and SHALL reserve JSON encoding for
non-string values. It MUST NOT wrap a string in JSON quotes before appending.

#### Scenario: Appending a string to a string
Given the key `k` holds the string `ab`, stored as raw UTF-8
When a client sends `POST /kv/k/append` with `{"value":"cd"}`
Then the stored value is `abcd`
And the reported length is 4

#### Scenario: Appending a non-string value
Given the key `k` does not exist
When a client sends `POST /kv/k/append` with `{"value":42}`
Then the value is appended in its JSON form `42`

### Requirement: SET with GET returns a previous string value
The REST `POST /kv/set` handler, when the request sets `get: true`, SHALL
return the previous value in `old_value` whenever one existed. A value stored
as raw UTF-8 SHALL be returned as a JSON string. The handler MUST NOT omit
`old_value` because the stored bytes failed to parse as JSON.

#### Scenario: Overwriting a plain string
Given the key `k` holds the string `first`
When a client sends `POST /kv/set` with `{"key":"k","value":"second","get":true}`
Then the response contains `"old_value":"first"`
And the response reports `"written":true`

#### Scenario: Overwriting a JSON-encoded value
Given the key `k` holds the value `{"a":1}`, stored JSON-encoded
When a client sends `POST /kv/set` with `{"key":"k","value":"x","get":true}`
Then `old_value` decodes to the object `{"a":1}` rather than to its source text

#### Scenario: No previous value
Given the key `k` does not exist
When a client sends `POST /kv/set` with `{"key":"k","value":"v","get":true}`
Then `old_value` is absent from the response

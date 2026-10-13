# Forward malformed counter amounts as JSON text so the server validates every transport

**Category**: sdk
**Tags**: phase26, counters, incrby, transport-parity, command-map

## Description

An SDK native map (SynapRPC/RESP3) must not decide on its own what a payload field
means when the HTTP envelope reads the same field. For `kv.incr`/`kv.decr` the map sends
`INCRBY`/`DECRBY` whenever `amount` is present and not null: a JSON integer travels as an
integer, and anything else travels as its JSON text (`"10"` with quotes, `1.5`, `true`).
The server then accepts only what HTTP accepts (a 64-bit integer) and refuses the rest
with no mutation, so one validator serves all three transports and no SDK silently
coerces a bad amount into 1. Integral float64 values (Go decodes JSON numbers as float64)
still marshal to plain integer text such as `10`, so they keep working.

Also check the typed module, not only the map: the C# and PHP modules sent the step as
`delta`, which HTTP never reads, and Python sent `kv.incrby`, which HTTP has no handler
for. Any field-name drift between the module and the envelope is the same bug.

## Example

fn counter_amount(payload: &Value) -> Option<WireValue> {
    match payload.get("amount") {
        None | Some(Value::Null) => None,
        Some(a) => Some(match a.as_i64() {
            Some(n) => WireValue::Int(n),
            None => WireValue::Str(a.to_string()),
        }),
    }
}

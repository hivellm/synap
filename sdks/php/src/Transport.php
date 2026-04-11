<?php

declare(strict_types=1);

namespace Synap\SDK;

/**
 * Transport mode constants.
 */
final class TransportMode
{
    /** SynapRPC binary protocol over TCP (default). Port 15501. */
    public const SYNAP_RPC = 'synaprpc';

    /** RESP3 Redis-compatible text protocol over TCP. Port 6379. */
    public const RESP3 = 'resp3';

    /** HTTP / StreamableHTTP fallback. Port 15500. */
    public const HTTP = 'http';
}

// ── Wire value helpers ─────────────────────────────────────────────────────────

/**
 * Wrap a PHP value in the externally-tagged WireValue envelope (rmp_serde format).
 *
 * Supported variants:
 *   null    → "Null"
 *   bool    → {"Bool": bool}
 *   int     → {"Int": int}
 *   float   → {"Float": float}
 *   string  → {"Str": string}
 *
 * @param mixed $v
 * @return mixed
 */
function toWireValue(mixed $v): mixed
{
    if ($v === null) {
        return 'Null';
    }
    if (is_bool($v)) {
        return ['Bool' => $v];
    }
    if (is_int($v)) {
        return ['Int' => $v];
    }
    if (is_float($v)) {
        return ['Float' => $v];
    }
    if (is_string($v)) {
        return ['Str' => $v];
    }
    // Fallback: stringify any other scalar
    return ['Str' => (string) $v];
}

/**
 * Unwrap a WireValue envelope back to a plain PHP value.
 *
 * Handles all externally-tagged variants produced by the Synap server:
 *   "Null"           → null
 *   {"Str": "x"}     → string
 *   {"Int": 42}      → int
 *   {"Float": 3.14}  → float
 *   {"Bool": true}   → bool
 *   {"Bytes": ...}   → raw bytes (string)
 *   {"Array": [...]} → list<mixed>
 *   {"Map": [[k,v]]} → assoc array
 *
 * @param mixed $wire
 * @return mixed
 */
function fromWireValue(mixed $wire): mixed
{
    if ($wire === 'Null' || $wire === null) {
        return null;
    }
    if (is_array($wire)) {
        if (isset($wire['Str'])) {
            return $wire['Str'];
        }
        if (isset($wire['Int'])) {
            return $wire['Int'];
        }
        if (isset($wire['Float'])) {
            return $wire['Float'];
        }
        if (isset($wire['Bool'])) {
            return $wire['Bool'];
        }
        if (isset($wire['Bytes'])) {
            return $wire['Bytes'];
        }
        if (isset($wire['Array'])) {
            return array_map(__NAMESPACE__ . '\\fromWireValue', $wire['Array']);
        }
        if (isset($wire['Map'])) {
            $result = [];
            foreach ($wire['Map'] as [$k, $v]) {
                $result[(string) fromWireValue($k)] = fromWireValue($v);
            }
            return $result;
        }
    }
    return $wire;
}

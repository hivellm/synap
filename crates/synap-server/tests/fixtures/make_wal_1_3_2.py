"""Generate wal_1_3_2.wal: a WAL in the on-disk format written by Synap 1.3.2.

Synap 1.3.2 (commit c92ea69) wrote a headerless stream of frames:

    len: u64 BE | crc32(payload): u32 BE | payload

where the payload is the bincode 2 `config::legacy()` encoding of
`WALEntry { offset: u64, timestamp: u64, operation: Operation }`: fixed-width
little-endian integers, u64 length prefixes for strings and sequences, a u32
variant index for enums, a one-byte tag for `Option`, one byte for `bool`.
The variant indices below are the declaration order of `Operation` in
crates/synap-server/src/persistence/types.rs at 1.3.2. The first offset of a
fresh 1.3.2 WAL is 1.

This encoder is independent of the Rust code on purpose, so the fixture checks
that 1.4.0 still reads what 1.3.2 wrote. Run from this directory:

    python make_wal_1_3_2.py
"""

import struct
import zlib

KV_SET = 0
KV_DEL = 1
HASH_SET = 7
HASH_INCR_BY = 9
LIST_PUSH = 11
SET_ADD = 18
Z_ADD = 24

TIMESTAMP = 1_760_000_000


def u32(v):
    return struct.pack("<I", v)


def u64(v):
    return struct.pack("<Q", v)


def i64(v):
    return struct.pack("<q", v)


def f64(v):
    return struct.pack("<d", v)


def flag(v):
    return b"\x01" if v else b"\x00"


def string(s):
    data = s.encode("utf-8")
    return u64(len(data)) + data


def blob(b):
    return u64(len(b)) + b


def seq(items, encode):
    return u64(len(items)) + b"".join(encode(i) for i in items)


def none():
    return b"\x00"


OPERATIONS = [
    u32(KV_SET) + string("legacy:a") + blob(b"alpha") + none(),
    u32(KV_SET) + string("legacy:gone") + blob(b"x") + none(),
    u32(KV_DEL) + seq(["legacy:gone"], string),
    u32(HASH_SET) + string("legacy:h") + string("f") + blob(b"hv"),
    u32(LIST_PUSH) + string("legacy:l") + seq([b"one", b"two"], blob) + flag(False),
    u32(SET_ADD) + string("legacy:s") + seq([b"m1", b"m2"], blob),
    u32(Z_ADD)
    + string("legacy:z")
    + blob(b"zm")
    + f64(2.5)
    + flag(False)
    + flag(False)
    + flag(False)
    + flag(False),
    u32(HASH_INCR_BY) + string("legacy:h") + string("n") + i64(7),
]


def frame(offset, operation):
    payload = u64(offset) + u64(TIMESTAMP) + operation
    return struct.pack(">Q", len(payload)) + struct.pack(">I", zlib.crc32(payload)) + payload


def main():
    data = b"".join(frame(i + 1, op) for i, op in enumerate(OPERATIONS))
    with open("wal_1_3_2.wal", "wb") as f:
        f.write(data)
    print(f"wrote wal_1_3_2.wal: {len(OPERATIONS)} frames, {len(data)} bytes")


if __name__ == "__main__":
    main()

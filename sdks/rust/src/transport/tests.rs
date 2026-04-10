use super::*;
use serde_json::json;

#[test]
fn wire_value_to_json_basic() {
    assert_eq!(WireValue::Null.to_json(), Value::Null);
    assert_eq!(WireValue::Bool(true).to_json(), json!(true));
    assert_eq!(WireValue::Int(42).to_json(), json!(42));
    assert_eq!(WireValue::Float(1.5).to_json(), json!(1.5));
    assert_eq!(WireValue::Str("hi".into()).to_json(), json!("hi"));
}

#[test]
fn wire_value_bytes_utf8() {
    let v = WireValue::Bytes(b"hello".to_vec());
    assert_eq!(v.to_json(), json!("hello"));
}

#[test]
fn map_command_kv_get() {
    let payload = json!({"key": "mykey"});
    let (cmd, args) = map_command("kv.get", &payload).unwrap();
    assert_eq!(cmd, "GET");
    assert_eq!(args, vec![WireValue::Str("mykey".into())]);
}

#[test]
fn map_command_kv_set_with_ttl() {
    let payload = json!({"key": "k", "value": "v", "ttl": 60});
    let (cmd, args) = map_command("kv.set", &payload).unwrap();
    assert_eq!(cmd, "SET");
    assert_eq!(
        args,
        vec![
            WireValue::Str("k".into()),
            WireValue::Str("v".into()),
            WireValue::Str("EX".into()),
            WireValue::Int(60),
        ]
    );
}

#[test]
fn map_command_kv_set_no_ttl() {
    let payload = json!({"key": "k", "value": "v", "ttl": null});
    let (cmd, args) = map_command("kv.set", &payload).unwrap();
    assert_eq!(cmd, "SET");
    assert_eq!(args.len(), 2);
}

#[test]
fn map_command_unknown_returns_none() {
    let payload = json!({});
    // Only truly unknown commands return None; queue/pubsub/stream are now mapped.
    assert!(map_command("queue.enqueue", &payload).is_none());
    assert!(map_command("completely.unknown", &payload).is_none());
    assert!(map_command("not.a.real.command", &payload).is_none());
}

#[test]
fn map_response_kv_del() {
    let v = map_response("kv.del", WireValue::Int(1));
    assert_eq!(v["deleted"], json!(true));

    let v = map_response("kv.del", WireValue::Int(0));
    assert_eq!(v["deleted"], json!(false));
}

#[test]
fn map_response_kv_exists() {
    let v = map_response("kv.exists", WireValue::Int(1));
    assert_eq!(v["exists"], json!(true));
}

#[test]
fn map_response_hash_getall() {
    let wire = WireValue::Array(vec![
        WireValue::Str("name".into()),
        WireValue::Str("Alice".into()),
        WireValue::Str("age".into()),
        WireValue::Str("30".into()),
    ]);
    let v = map_response("hash.getall", wire);
    assert_eq!(v["fields"]["name"], json!("Alice"));
    assert_eq!(v["fields"]["age"], json!("30"));
}

#[test]
fn map_response_zrange_withscores() {
    let wire = WireValue::Array(vec![
        WireValue::Str("player1".into()),
        WireValue::Str("100.0".into()),
        WireValue::Str("player2".into()),
        WireValue::Str("200.0".into()),
    ]);
    let v = map_response("sortedset.zrange", wire);
    let members = v["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["member"], json!("player1"));
    assert_eq!(members[0]["score"], json!(100.0));
}

#[test]
fn map_command_hash_mset_hashmap_format() {
    let mut map = serde_json::Map::new();
    map.insert("f1".into(), json!("v1"));
    map.insert("f2".into(), json!("v2"));
    let payload = json!({"key": "k", "fields": Value::Object(map)});
    let (cmd, args) = map_command("hash.mset", &payload).unwrap();
    assert_eq!(cmd, "HSET");
    assert!(args.len() >= 3); // key + at least one field/value pair
}

#[test]
fn map_command_set_add_multi() {
    let payload = json!({"key": "myset", "members": ["a", "b", "c"]});
    let (cmd, args) = map_command("set.add", &payload).unwrap();
    assert_eq!(cmd, "SADD");
    assert_eq!(args.len(), 4); // key + 3 members
}

#[test]
fn wire_value_roundtrip_msgpack() {
    let vals = vec![
        WireValue::Null,
        WireValue::Bool(true),
        WireValue::Int(i64::MAX),
        WireValue::Float(1.5),
        WireValue::Str("hello".into()),
        WireValue::Array(vec![WireValue::Int(1), WireValue::Str("two".into())]),
    ];
    for v in vals {
        let enc = rmp_serde::to_vec(&v).unwrap();
        let dec: WireValue = rmp_serde::from_slice(&enc).unwrap();
        assert_eq!(v, dec);
    }
}

// ── WireValue helpers ─────────────────────────────────────────────────────

#[test]
fn wire_value_as_str() {
    assert_eq!(WireValue::Str("hello".into()).as_str(), Some("hello"));
    assert_eq!(WireValue::Int(1).as_str(), None);
    assert_eq!(WireValue::Null.as_str(), None);
}

#[test]
fn wire_value_as_int() {
    assert_eq!(WireValue::Int(99).as_int(), Some(99));
    assert_eq!(WireValue::Int(-1).as_int(), Some(-1));
    assert_eq!(WireValue::Str("x".into()).as_int(), None);
    assert_eq!(WireValue::Null.as_int(), None);
}

#[test]
fn wire_value_as_float() {
    assert_eq!(WireValue::Float(3.125).as_float(), Some(3.125));
    // Str that parses as float
    assert_eq!(WireValue::Str("2.5".into()).as_float(), Some(2.5));
    assert_eq!(WireValue::Str("not-a-float".into()).as_float(), None);
    assert_eq!(WireValue::Null.as_float(), None);
}

#[test]
fn wire_value_is_null() {
    assert!(WireValue::Null.is_null());
    assert!(!WireValue::Int(0).is_null());
    assert!(!WireValue::Bool(false).is_null());
}

#[test]
fn wire_value_to_json_array_and_map() {
    let arr = WireValue::Array(vec![WireValue::Int(1), WireValue::Str("two".into())]);
    let j = arr.to_json();
    assert_eq!(j[0], json!(1));
    assert_eq!(j[1], json!("two"));

    let map = WireValue::Map(vec![(WireValue::Str("key".into()), WireValue::Int(42))]);
    let j = map.to_json();
    assert_eq!(j["key"], json!(42));
}

#[test]
fn wire_value_bytes_non_utf8_renders_hex() {
    let v = WireValue::Bytes(vec![0xFF, 0xFE]);
    let j = v.to_json();
    // Non-UTF8 bytes should become a hex string.
    assert!(j.as_str().unwrap().chars().all(|c| c.is_ascii_hexdigit()));
}

// ── map_command – additional commands ─────────────────────────────────────

#[test]
fn map_command_kv_delete() {
    let payload = json!({"key": "testkey"});
    let (cmd, args) = map_command("kv.del", &payload).unwrap();
    assert_eq!(cmd, "DEL");
    assert_eq!(args, vec![WireValue::Str("testkey".into())]);
}

#[test]
fn map_command_kv_exists() {
    let payload = json!({"key": "testkey"});
    let (cmd, args) = map_command("kv.exists", &payload).unwrap();
    assert_eq!(cmd, "EXISTS");
    assert_eq!(args, vec![WireValue::Str("testkey".into())]);
}

#[test]
fn map_command_kv_incr() {
    let payload = json!({"key": "counter"});
    let (cmd, args) = map_command("kv.incr", &payload).unwrap();
    assert_eq!(cmd, "INCR");
    assert_eq!(args, vec![WireValue::Str("counter".into())]);
}

#[test]
fn map_command_hash_get() {
    let payload = json!({"key": "myhash", "field": "f1"});
    let (cmd, args) = map_command("hash.get", &payload).unwrap();
    assert_eq!(cmd, "HGET");
    assert_eq!(
        args,
        vec![WireValue::Str("myhash".into()), WireValue::Str("f1".into()),]
    );
}

#[test]
fn map_command_list_lpush() {
    let payload = json!({"key": "mylist", "values": ["a", "b"]});
    let (cmd, args) = map_command("list.lpush", &payload).unwrap();
    assert_eq!(cmd, "LPUSH");
    assert_eq!(
        args,
        vec![
            WireValue::Str("mylist".into()),
            WireValue::Str("a".into()),
            WireValue::Str("b".into()),
        ]
    );
}

#[test]
fn map_command_set_add() {
    let payload = json!({"key": "myset", "members": ["x"]});
    let (cmd, args) = map_command("set.add", &payload).unwrap();
    assert_eq!(cmd, "SADD");
    assert_eq!(
        args,
        vec![WireValue::Str("myset".into()), WireValue::Str("x".into()),]
    );
}

#[test]
fn map_command_queue_publish_returns_some() {
    // queue.publish is now natively mapped to QPUBLISH
    assert!(map_command("queue.publish", &json!({"queue_name": "q", "payload": []})).is_some());
}

#[test]
fn map_command_stream_publish_returns_some() {
    // stream.publish is now natively mapped to SPUBLISH
    let result = map_command(
        "stream.publish",
        &json!({"room": "r", "event": "e", "data": "bytes"}),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().0, "SPUBLISH");
}

// ── map_response – additional cases ──────────────────────────────────────

#[test]
fn map_response_kv_get_passes_through() {
    let wire = WireValue::Str("myvalue".into());
    let v = map_response("kv.get", wire);
    assert_eq!(v, json!("myvalue"));
}

#[test]
fn map_response_kv_get_null_passes_through() {
    let v = map_response("kv.get", WireValue::Null);
    assert_eq!(v, Value::Null);
}

#[test]
fn map_response_kv_del_deleted_false_when_zero() {
    let v = map_response("kv.del", WireValue::Int(0));
    assert_eq!(v["deleted"], json!(false));
}

#[test]
fn map_response_kv_exists_false_when_zero() {
    let v = map_response("kv.exists", WireValue::Int(0));
    assert_eq!(v["exists"], json!(false));
}

#[test]
fn map_response_kv_incr() {
    let v = map_response("kv.incr", WireValue::Int(5));
    assert_eq!(v["value"], json!(5));
}

#[test]
fn map_response_hash_get_value() {
    let v = map_response("hash.get", WireValue::Str("hello".into()));
    assert_eq!(v["value"], json!("hello"));
}

#[test]
fn map_response_hash_get_null() {
    let v = map_response("hash.get", WireValue::Null);
    assert_eq!(v["value"], Value::Null);
}

#[test]
fn map_response_list_lpush_length() {
    let v = map_response("list.lpush", WireValue::Int(3));
    assert_eq!(v["length"], json!(3));
}

#[test]
fn map_response_set_add_added_count() {
    let v = map_response("set.add", WireValue::Int(2));
    assert_eq!(v["added"], json!(2));
}

// ── SynapRpcTransport – real TCP round-trip ───────────────────────────────

/// Start a minimal SynapRPC echo server that handles exactly one request.
///
/// The server reads a 4-byte LE length prefix, deserialises the msgpack
/// `RpcRequest`, then sends back an `RpcResponse` with `Ok(WireValue::Str("testvalue"))`.
async fn run_synap_rpc_server_once(listener: tokio::net::TcpListener, expected_cmd: &'static str) {
    let (mut stream, _) = listener.accept().await.expect("accept");

    // Read 4-byte length prefix.
    let mut len_buf = [0u8; 4];
    tokio::io::AsyncReadExt::read_exact(&mut stream, &mut len_buf)
        .await
        .expect("read len");
    let body_len = u32::from_le_bytes(len_buf) as usize;

    // Read msgpack body.
    let mut body = vec![0u8; body_len];
    tokio::io::AsyncReadExt::read_exact(&mut stream, &mut body)
        .await
        .expect("read body");

    let req: RpcRequest = rmp_serde::from_slice(&body).expect("decode request");
    assert_eq!(req.command, expected_cmd);

    // Build response.
    let resp = RpcResponse {
        id: req.id,
        result: Ok(WireValue::Str("testvalue".into())),
    };
    let resp_bytes = rmp_serde::to_vec(&resp).expect("encode response");
    let resp_len = (resp_bytes.len() as u32).to_le_bytes();

    tokio::io::AsyncWriteExt::write_all(&mut stream, &resp_len)
        .await
        .expect("write len");
    tokio::io::AsyncWriteExt::write_all(&mut stream, &resp_bytes)
        .await
        .expect("write body");
}

#[tokio::test]
async fn synap_rpc_transport_execute_get() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("local_addr").port();

    tokio::spawn(run_synap_rpc_server_once(listener, "GET"));

    let transport = SynapRpcTransport::new("127.0.0.1", port, Duration::from_secs(5));
    let result = transport
        .execute("GET", vec![WireValue::Str("testkey".into())])
        .await
        .expect("execute");

    assert_eq!(result, WireValue::Str("testvalue".into()));
}

#[tokio::test]
async fn synap_rpc_transport_command_uppercased() {
    // Verifies that the transport converts the command to uppercase before
    // sending, so the server receives "SET" not "set".
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("local_addr").port();

    tokio::spawn(run_synap_rpc_server_once(listener, "SET"));

    let transport = SynapRpcTransport::new("127.0.0.1", port, Duration::from_secs(5));
    let _ = transport
        .execute(
            "set",
            vec![WireValue::Str("k".into()), WireValue::Str("v".into())],
        )
        .await
        .expect("execute");
}

// ── Resp3Transport – real TCP round-trip ──────────────────────────────────

/// Start a minimal RESP3 server that responds to HELLO and one GET command.
async fn run_resp3_server_once(listener: tokio::net::TcpListener) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (stream, _) = listener.accept().await.expect("accept");
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);

    // Read lines and respond to the RESP2 multibulk frame.
    // The transport sends: *2\r\n$3\r\nGET\r\n$7\r\ntestkey\r\n
    // We parse the count line then skip arg lines, replying after each command.
    let mut buf = String::new();
    loop {
        buf.clear();
        let n = reader.read_line(&mut buf).await.expect("read");
        if n == 0 {
            break;
        }
        let line = buf.trim_end_matches(['\r', '\n']);
        if let Some(rest) = line.strip_prefix('*') {
            // Multibulk: read count args then skip them.
            let argc: usize = rest.parse().unwrap_or(0);
            for _ in 0..argc {
                // Skip "$len\r\n" line.
                buf.clear();
                reader.read_line(&mut buf).await.expect("read len line");
                let len_line = buf.trim_end_matches(['\r', '\n']);
                if let Some(len_str) = len_line.strip_prefix('$') {
                    let len: usize = len_str.parse().unwrap_or(0);
                    // Read bulk data + CRLF.
                    let mut data = vec![0u8; len + 2];
                    tokio::io::AsyncReadExt::read_exact(&mut reader, &mut data)
                        .await
                        .expect("read bulk");
                }
            }
            // Respond with a bulk string.
            w.write_all(b"$9\r\ntestvalue\r\n")
                .await
                .expect("write response");
            break;
        }
    }
}

#[tokio::test]
async fn resp3_transport_execute_get() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("local_addr").port();

    tokio::spawn(run_resp3_server_once(listener));

    let transport = Resp3Transport::new("127.0.0.1", port, Duration::from_secs(5));
    let result = transport
        .execute("GET", vec![WireValue::Str("testkey".into())])
        .await
        .expect("execute");

    assert_eq!(result, WireValue::Str("testvalue".into()));
}

// ── wire_value_to_resp_bytes ──────────────────────────────────────────────

#[test]
fn wire_value_to_resp_bytes_variants() {
    assert_eq!(wire_value_to_resp_bytes(&WireValue::Null), b"".to_vec());
    assert_eq!(
        wire_value_to_resp_bytes(&WireValue::Bool(true)),
        b"1".to_vec()
    );
    assert_eq!(
        wire_value_to_resp_bytes(&WireValue::Bool(false)),
        b"0".to_vec()
    );
    assert_eq!(
        wire_value_to_resp_bytes(&WireValue::Int(42)),
        b"42".to_vec()
    );
    assert_eq!(
        wire_value_to_resp_bytes(&WireValue::Str("hello".into())),
        b"hello".to_vec()
    );
    assert_eq!(
        wire_value_to_resp_bytes(&WireValue::Bytes(vec![1, 2, 3])),
        vec![1u8, 2, 3]
    );
    // Arrays and Maps render as empty bytes.
    assert_eq!(
        wire_value_to_resp_bytes(&WireValue::Array(vec![])),
        b"".to_vec()
    );
    assert_eq!(
        wire_value_to_resp_bytes(&WireValue::Map(vec![])),
        b"".to_vec()
    );
}

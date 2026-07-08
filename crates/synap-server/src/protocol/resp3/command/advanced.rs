use super::{AppState, Resp3Value, arg_bytes, arg_f64, arg_i64, arg_str, arg_u64, err_wrong_args};
use crate::core::geospatial::DistanceUnit;
use crate::scripting::ScriptExecContext;

// ── Queue commands (3.1) ──────────────────────────────────────────────────────

pub(super) async fn cmd_qcreate(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("QCREATE");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.create_queue(&name, None).await {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qdelete(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("QDELETE");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.delete_queue(&name).await {
        Ok(true) => Resp3Value::Integer(1),
        Ok(false) => Resp3Value::Integer(0),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qlist(state: &AppState) -> Resp3Value {
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.list_queues().await {
        Ok(names) => Resp3Value::Array(
            names
                .into_iter()
                .map(|n| Resp3Value::BulkString(n.into_bytes()))
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qpublish(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("QPUBLISH");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let payload = match arg_bytes(args, 2) {
        Some(p) => p,
        None => return Resp3Value::Error("ERR payload required".into()),
    };
    // Optional: PRIORITY <n>
    let priority = if args.len() >= 5 {
        match args[3].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("PRIORITY") => arg_str(args, 4).and_then(|s| s.parse::<u8>().ok()),
            _ => None,
        }
    } else {
        None
    };
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.publish(&name, payload, priority, None).await {
        Ok(id) => Resp3Value::BulkString(id.into_bytes()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qconsume(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("QCONSUME");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let consumer_id = arg_str(args, 2).unwrap_or_else(|| "resp3".into());
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.consume(&name, &consumer_id).await {
        Ok(Some(msg)) => Resp3Value::Array(vec![
            Resp3Value::BulkString(msg.id.as_bytes().to_vec()),
            Resp3Value::BulkString((*msg.payload).clone()),
            Resp3Value::Integer(msg.priority as i64),
            Resp3Value::Integer(msg.retry_count as i64),
        ]),
        Ok(None) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qack(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("QACK");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let msg_id = match arg_str(args, 2) {
        Some(id) => id,
        None => return Resp3Value::Error("ERR message id must be a string".into()),
    };
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.ack(&name, &msg_id).await {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qnack(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("QNACK");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let msg_id = match arg_str(args, 2) {
        Some(id) => id,
        None => return Resp3Value::Error("ERR message id must be a string".into()),
    };
    // Optional: REQUEUE 1|0
    let requeue = if args.len() >= 5 {
        match args[3].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("REQUEUE") => arg_i64(args, 4).map(|n| n != 0).unwrap_or(true),
            _ => true,
        }
    } else {
        true
    };
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.nack(&name, &msg_id, requeue).await {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qstats(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("QSTATS");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.stats(&name).await {
        Ok(stats) => Resp3Value::Array(vec![
            Resp3Value::BulkString(b"depth".to_vec()),
            Resp3Value::Integer(stats.depth as i64),
            Resp3Value::BulkString(b"consumers".to_vec()),
            Resp3Value::Integer(stats.consumers as i64),
            Resp3Value::BulkString(b"published".to_vec()),
            Resp3Value::Integer(stats.published as i64),
            Resp3Value::BulkString(b"consumed".to_vec()),
            Resp3Value::Integer(stats.consumed as i64),
            Resp3Value::BulkString(b"acked".to_vec()),
            Resp3Value::Integer(stats.acked as i64),
            Resp3Value::BulkString(b"nacked".to_vec()),
            Resp3Value::Integer(stats.nacked as i64),
            Resp3Value::BulkString(b"dead_lettered".to_vec()),
            Resp3Value::Integer(stats.dead_lettered as i64),
        ]),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_qpurge(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("QPURGE");
    }
    let name = match arg_str(args, 1) {
        Some(n) => n,
        None => return Resp3Value::Error("ERR queue name must be a string".into()),
    };
    let qm = match state.queue_manager.as_ref() {
        Some(qm) => qm,
        None => return Resp3Value::Error("ERR queue subsystem not enabled".into()),
    };
    match qm.purge(&name).await {
        Ok(count) => Resp3Value::Integer(count as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── Stream commands (3.2) ─────────────────────────────────────────────────────

/// `XADD <room> * <event_type> <data> [field value ...]`
pub(super) async fn cmd_xadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    // Minimum: XADD <room> <id> <field> <value>
    if args.len() < 5 {
        return err_wrong_args("XADD");
    }
    let room = match arg_str(args, 1) {
        Some(r) => r,
        None => return Resp3Value::Error("ERR room name must be a string".into()),
    };
    // args[2] = id (usually "*" for auto-generate — we ignore it)
    let event_type = match arg_str(args, 3) {
        Some(f) => f,
        None => return Resp3Value::Error("ERR field name must be a string".into()),
    };
    let data = arg_bytes(args, 4).unwrap_or_default();
    let sm = match state.stream_manager.as_ref() {
        Some(sm) => sm,
        None => return Resp3Value::Error("ERR stream subsystem not enabled".into()),
    };
    match sm.publish(&room, &event_type, data).await {
        Ok(offset) => Resp3Value::BulkString(offset.to_string().into_bytes()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `XREAD [COUNT <n>] [BLOCK <ms>] STREAMS <room> <offset>`
pub(super) async fn cmd_xread(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("XREAD");
    }
    let mut i = 1;
    let mut limit = 10usize;
    while i < args.len() {
        match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("COUNT") => {
                limit = arg_u64(args, i + 1).unwrap_or(10) as usize;
                i += 2;
            }
            Some("BLOCK") => i += 2, // ignore block timeout
            Some("STREAMS") => {
                i += 1;
                break;
            }
            _ => i += 1,
        }
    }
    if i + 1 >= args.len() {
        return err_wrong_args("XREAD");
    }
    let room = match arg_str(args, i) {
        Some(r) => r,
        None => return Resp3Value::Error("ERR room name must be a string".into()),
    };
    let from_offset = arg_u64(args, i + 1).unwrap_or(0);
    let sm = match state.stream_manager.as_ref() {
        Some(sm) => sm,
        None => return Resp3Value::Error("ERR stream subsystem not enabled".into()),
    };
    match sm.consume(&room, "resp3", from_offset, limit).await {
        Ok(events) => Resp3Value::Array(
            events
                .into_iter()
                .map(|ev| {
                    Resp3Value::Array(vec![
                        Resp3Value::BulkString(ev.offset.to_string().into_bytes()),
                        Resp3Value::Array(vec![
                            Resp3Value::BulkString(ev.event.into_bytes()),
                            Resp3Value::BulkString(ev.data),
                        ]),
                    ])
                })
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `XREADGROUP GROUP <group> <consumer> [COUNT <n>] STREAMS <room> <offset>`
pub(super) async fn cmd_xreadgroup(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 7 {
        return err_wrong_args("XREADGROUP");
    }
    let consumer = arg_str(args, 3).unwrap_or_else(|| "resp3".into());
    let mut i = 4;
    let mut limit = 10usize;
    while i < args.len() {
        match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("COUNT") => {
                limit = arg_u64(args, i + 1).unwrap_or(10) as usize;
                i += 2;
            }
            Some("STREAMS") => {
                i += 1;
                break;
            }
            _ => i += 1,
        }
    }
    if i + 1 >= args.len() {
        return err_wrong_args("XREADGROUP");
    }
    let room = match arg_str(args, i) {
        Some(r) => r,
        None => return Resp3Value::Error("ERR room name must be a string".into()),
    };
    let from_offset = arg_u64(args, i + 1).unwrap_or(0);
    let sm = match state.stream_manager.as_ref() {
        Some(sm) => sm,
        None => return Resp3Value::Error("ERR stream subsystem not enabled".into()),
    };
    match sm.consume(&room, &consumer, from_offset, limit).await {
        Ok(events) => Resp3Value::Array(
            events
                .into_iter()
                .map(|ev| {
                    Resp3Value::Array(vec![
                        Resp3Value::BulkString(ev.offset.to_string().into_bytes()),
                        Resp3Value::Array(vec![
                            Resp3Value::BulkString(ev.event.into_bytes()),
                            Resp3Value::BulkString(ev.data),
                        ]),
                    ])
                })
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `XRANGE <room> <start> <end> [COUNT <n>]`
pub(super) async fn cmd_xrange(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("XRANGE");
    }
    let room = match arg_str(args, 1) {
        Some(r) => r,
        None => return Resp3Value::Error("ERR room name must be a string".into()),
    };
    let from_offset = match arg_str(args, 2).as_deref() {
        Some("-") => 0u64,
        _ => arg_u64(args, 2).unwrap_or(0),
    };
    let limit = if args.len() >= 6 {
        match args[4].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("COUNT") => arg_u64(args, 5).unwrap_or(100) as usize,
            _ => 100,
        }
    } else {
        100
    };
    let sm = match state.stream_manager.as_ref() {
        Some(sm) => sm,
        None => return Resp3Value::Error("ERR stream subsystem not enabled".into()),
    };
    match sm.consume(&room, "resp3-range", from_offset, limit).await {
        Ok(events) => Resp3Value::Array(
            events
                .into_iter()
                .map(|ev| {
                    Resp3Value::Array(vec![
                        Resp3Value::BulkString(ev.offset.to_string().into_bytes()),
                        Resp3Value::Array(vec![
                            Resp3Value::BulkString(ev.event.into_bytes()),
                            Resp3Value::BulkString(ev.data),
                        ]),
                    ])
                })
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `XDEL <room> <id> [id ...]` — append-only model; always returns 0
pub(super) async fn cmd_xdel(_state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("XDEL");
    }
    Resp3Value::Integer(0)
}

/// `XINFO STREAM <room>` or `XINFO ROOMS`
pub(super) async fn cmd_xinfo(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("XINFO");
    }
    let sub = args[1]
        .as_str()
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();
    let sm = match state.stream_manager.as_ref() {
        Some(sm) => sm,
        None => return Resp3Value::Error("ERR stream subsystem not enabled".into()),
    };
    match sub.as_str() {
        "ROOMS" => {
            let rooms = sm.list_rooms().await;
            Resp3Value::Array(
                rooms
                    .into_iter()
                    .map(|r| Resp3Value::BulkString(r.into_bytes()))
                    .collect(),
            )
        }
        "STREAM" => {
            if args.len() < 3 {
                return err_wrong_args("XINFO STREAM");
            }
            let room = match arg_str(args, 2) {
                Some(r) => r,
                None => return Resp3Value::Error("ERR room name must be a string".into()),
            };
            match sm.room_stats(&room).await {
                Ok(stats) => Resp3Value::Array(vec![
                    Resp3Value::BulkString(b"name".to_vec()),
                    Resp3Value::BulkString(stats.name.into_bytes()),
                    Resp3Value::BulkString(b"message-count".to_vec()),
                    Resp3Value::Integer(stats.message_count as i64),
                    Resp3Value::BulkString(b"first-offset".to_vec()),
                    Resp3Value::Integer(stats.min_offset as i64),
                    Resp3Value::BulkString(b"last-offset".to_vec()),
                    Resp3Value::Integer(stats.max_offset as i64),
                    Resp3Value::BulkString(b"subscribers".to_vec()),
                    Resp3Value::Integer(stats.subscriber_count as i64),
                ]),
                Err(e) => Resp3Value::Error(format!("ERR {e}")),
            }
        }
        _ => Resp3Value::Error(format!(
            "ERR unknown XINFO subcommand '{}'. Try STREAM or ROOMS",
            sub
        )),
    }
}

/// `XACK <room> <group> <id> [id ...]` — returns count of IDs acknowledged
pub(super) async fn cmd_xack(_state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("XACK");
    }
    // Append-only stream model; acknowledge positionally
    Resp3Value::Integer((args.len() - 3) as i64)
}

// ── Pub/Sub commands (3.3) ────────────────────────────────────────────────────

pub(super) async fn cmd_publish(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("PUBLISH");
    }
    let topic = match arg_str(args, 1) {
        Some(t) => t,
        None => return Resp3Value::Error("ERR topic must be a string".into()),
    };
    let payload = match arg_bytes(args, 2) {
        Some(p) => p,
        None => return Resp3Value::Error("ERR payload required".into()),
    };
    let ps = match state.pubsub_router.as_ref() {
        Some(ps) => ps,
        None => return Resp3Value::Error("ERR pubsub subsystem not enabled".into()),
    };
    let json_payload = serde_json::Value::String(
        String::from_utf8(payload)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
    );
    match ps.publish(&topic, json_payload, None) {
        Ok(result) => Resp3Value::Integer(result.subscribers_matched as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_subscribe(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("SUBSCRIBE");
    }
    let topics: Vec<String> = (1..args.len()).filter_map(|i| arg_str(args, i)).collect();
    if topics.is_empty() {
        return err_wrong_args("SUBSCRIBE");
    }
    let ps = match state.pubsub_router.as_ref() {
        Some(ps) => ps,
        None => return Resp3Value::Error("ERR pubsub subsystem not enabled".into()),
    };
    match ps.subscribe(topics) {
        Ok(result) => Resp3Value::Integer(result.subscription_count as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_unsubscribe(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    let ps = match state.pubsub_router.as_ref() {
        Some(ps) => ps,
        None => return Resp3Value::Error("ERR pubsub subsystem not enabled".into()),
    };
    let topics = if args.len() >= 2 {
        let t: Vec<String> = (1..args.len()).filter_map(|i| arg_str(args, i)).collect();
        if t.is_empty() { None } else { Some(t) }
    } else {
        None
    };
    match ps.unsubscribe("resp3", topics) {
        Ok(removed) => Resp3Value::Integer(removed as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_psubscribe(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("PSUBSCRIBE");
    }
    let topics: Vec<String> = (1..args.len()).filter_map(|i| arg_str(args, i)).collect();
    let ps = match state.pubsub_router.as_ref() {
        Some(ps) => ps,
        None => return Resp3Value::Error("ERR pubsub subsystem not enabled".into()),
    };
    match ps.subscribe(topics) {
        Ok(result) => Resp3Value::Integer(result.subscription_count as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

pub(super) async fn cmd_pubsub(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("PUBSUB");
    }
    let sub = args[1]
        .as_str()
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();
    let ps = match state.pubsub_router.as_ref() {
        Some(ps) => ps,
        None => return Resp3Value::Error("ERR pubsub subsystem not enabled".into()),
    };
    match sub.as_str() {
        "CHANNELS" => {
            let topics = ps.list_topics();
            Resp3Value::Array(
                topics
                    .into_iter()
                    .map(|t| Resp3Value::BulkString(t.into_bytes()))
                    .collect(),
            )
        }
        "NUMSUB" => {
            let topics = ps.list_topics();
            let mut out = Vec::with_capacity(topics.len() * 2);
            for t in topics {
                let count = ps
                    .get_topic_info(&t)
                    .map(|i| i.subscriber_count)
                    .unwrap_or(0);
                out.push(Resp3Value::BulkString(t.into_bytes()));
                out.push(Resp3Value::Integer(count as i64));
            }
            Resp3Value::Array(out)
        }
        "NUMPAT" => Resp3Value::Integer(0),
        _ => Resp3Value::Error(format!(
            "ERR unknown PUBSUB subcommand '{}'. Try CHANNELS, NUMSUB, NUMPAT",
            sub
        )),
    }
}

// ── Transaction commands (3.4) ────────────────────────────────────────────────

/// `MULTI <client_id>`
pub(super) async fn cmd_multi(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("MULTI");
    }
    let client_id = match arg_str(args, 1) {
        Some(id) => id,
        None => return Resp3Value::Error("ERR client_id must be a string".into()),
    };
    match state.transaction_manager.multi(client_id) {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `EXEC <client_id>`
pub(super) async fn cmd_exec(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("EXEC");
    }
    let client_id = match arg_str(args, 1) {
        Some(id) => id,
        None => return Resp3Value::Error("ERR client_id must be a string".into()),
    };
    match state.transaction_manager.exec(&client_id).await {
        Ok(Some(results)) => Resp3Value::Array(
            results
                .into_iter()
                .map(|v| Resp3Value::BulkString(v.to_string().into_bytes()))
                .collect(),
        ),
        Ok(None) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `DISCARD <client_id>`
pub(super) async fn cmd_discard(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("DISCARD");
    }
    let client_id = match arg_str(args, 1) {
        Some(id) => id,
        None => return Resp3Value::Error("ERR client_id must be a string".into()),
    };
    match state.transaction_manager.discard(&client_id) {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `WATCH <client_id> <key> [key ...]`
pub(super) async fn cmd_watch(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("WATCH");
    }
    let client_id = match arg_str(args, 1) {
        Some(id) => id,
        None => return Resp3Value::Error("ERR client_id must be a string".into()),
    };
    let keys: Vec<String> = (2..args.len()).filter_map(|i| arg_str(args, i)).collect();
    if keys.is_empty() {
        return err_wrong_args("WATCH");
    }
    match state.transaction_manager.watch(&client_id, keys) {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `UNWATCH <client_id>`
pub(super) async fn cmd_unwatch(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("UNWATCH");
    }
    let client_id = match arg_str(args, 1) {
        Some(id) => id,
        None => return Resp3Value::Error("ERR client_id must be a string".into()),
    };
    match state.transaction_manager.unwatch(&client_id) {
        Ok(()) => Resp3Value::SimpleString("OK".into()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

// ── Script commands (3.5) ─────────────────────────────────────────────────────

/// `EVAL <script> <numkeys> [key ...] [arg ...]`
pub(super) async fn cmd_eval(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("EVAL");
    }
    let script = match arg_str(args, 1) {
        Some(s) => s,
        None => return Resp3Value::Error("ERR script must be a string".into()),
    };
    let numkeys = match arg_u64(args, 2) {
        Some(n) => n as usize,
        None => return Resp3Value::Error("ERR numkeys must be an integer".into()),
    };
    let key_end = (3 + numkeys).min(args.len());
    let keys: Vec<String> = (3..key_end).filter_map(|i| arg_str(args, i)).collect();
    let script_args: Vec<String> = (key_end..args.len())
        .filter_map(|i| arg_str(args, i))
        .collect();
    let ctx = ScriptExecContext {
        kv_store: state.kv_store.clone(),
        hash_store: state.hash_store.clone(),
        list_store: state.list_store.clone(),
        set_store: state.set_store.clone(),
        sorted_set_store: state.sorted_set_store.clone(),
    };
    match state
        .script_manager
        .eval(ctx, &script, keys, script_args, None)
        .await
    {
        Ok((v, _sha)) => Resp3Value::BulkString(v.to_string().into_bytes()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `EVALSHA <sha> <numkeys> [key ...] [arg ...]`
pub(super) async fn cmd_evalsha(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("EVALSHA");
    }
    let sha = match arg_str(args, 1) {
        Some(s) => s,
        None => return Resp3Value::Error("ERR sha must be a string".into()),
    };
    let numkeys = match arg_u64(args, 2) {
        Some(n) => n as usize,
        None => return Resp3Value::Error("ERR numkeys must be an integer".into()),
    };
    let key_end = (3 + numkeys).min(args.len());
    let keys: Vec<String> = (3..key_end).filter_map(|i| arg_str(args, i)).collect();
    let script_args: Vec<String> = (key_end..args.len())
        .filter_map(|i| arg_str(args, i))
        .collect();
    let ctx = ScriptExecContext {
        kv_store: state.kv_store.clone(),
        hash_store: state.hash_store.clone(),
        list_store: state.list_store.clone(),
        set_store: state.set_store.clone(),
        sorted_set_store: state.sorted_set_store.clone(),
    };
    match state
        .script_manager
        .evalsha(ctx, &sha, keys, script_args, None)
        .await
    {
        Ok(v) => Resp3Value::BulkString(v.to_string().into_bytes()),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `SCRIPT LOAD|EXISTS|FLUSH|KILL ...`
pub(super) async fn cmd_script(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 2 {
        return err_wrong_args("SCRIPT");
    }
    let sub = args[1]
        .as_str()
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();
    match sub.as_str() {
        "LOAD" => {
            if args.len() < 3 {
                return err_wrong_args("SCRIPT LOAD");
            }
            let source = match arg_str(args, 2) {
                Some(s) => s,
                None => return Resp3Value::Error("ERR script must be a string".into()),
            };
            let sha = state.script_manager.load_script(&source);
            Resp3Value::BulkString(sha.into_bytes())
        }
        "EXISTS" => {
            let hashes: Vec<String> = (2..args.len()).filter_map(|i| arg_str(args, i)).collect();
            let results = state.script_manager.script_exists(&hashes);
            Resp3Value::Array(
                results
                    .into_iter()
                    .map(|b| Resp3Value::Integer(b as i64))
                    .collect(),
            )
        }
        "FLUSH" => {
            state.script_manager.flush();
            Resp3Value::SimpleString("OK".into())
        }
        "KILL" => {
            if state.script_manager.kill_running() {
                Resp3Value::SimpleString("OK".into())
            } else {
                Resp3Value::Error("ERR No scripts in execution right now.".into())
            }
        }
        _ => Resp3Value::Error(format!(
            "ERR unknown SCRIPT subcommand '{}'. Try LOAD, EXISTS, FLUSH, KILL",
            sub
        )),
    }
}

// ── Geospatial commands (3.7) ─────────────────────────────────────────────────

fn geo_results_to_resp3(
    results: Vec<crate::core::geospatial::GeospatialRadiusResult>,
) -> Resp3Value {
    Resp3Value::Array(
        results
            .into_iter()
            .map(|(member, dist, coord)| {
                if dist.is_none() && coord.is_none() {
                    Resp3Value::BulkString(member)
                } else {
                    Resp3Value::Array(vec![
                        Resp3Value::BulkString(member),
                        dist.map(|d| Resp3Value::BulkString(format!("{d:.4}").into_bytes()))
                            .unwrap_or(Resp3Value::Null),
                        coord
                            .map(|c| {
                                // Redis convention: longitude first, then latitude
                                Resp3Value::Array(vec![
                                    Resp3Value::BulkString(format!("{:.6}", c.lon).into_bytes()),
                                    Resp3Value::BulkString(format!("{:.6}", c.lat).into_bytes()),
                                ])
                            })
                            .unwrap_or(Resp3Value::Null),
                    ])
                }
            })
            .collect(),
    )
}

/// `GEOADD key [NX|XX] [CH] longitude latitude member [...]`
pub(super) async fn cmd_geoadd(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 5 {
        return err_wrong_args("GEOADD");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let mut nx = false;
    let mut xx = false;
    let mut ch = false;
    let mut pos = 2usize;
    while pos < args.len() {
        match args[pos]
            .as_str()
            .map(|s| s.to_ascii_uppercase())
            .as_deref()
        {
            Some("NX") => {
                nx = true;
                pos += 1;
            }
            Some("XX") => {
                xx = true;
                pos += 1;
            }
            Some("CH") => {
                ch = true;
                pos += 1;
            }
            _ => break,
        }
    }
    if args.len() <= pos || (args.len() - pos) % 3 != 0 {
        return Resp3Value::Error(
            "ERR syntax error: expected longitude latitude member triplets".into(),
        );
    }
    let mut locations = Vec::new();
    while pos + 2 < args.len() {
        // Redis wire: longitude first, latitude second — internal API uses (lat, lon)
        let lon = match arg_f64(args, pos) {
            Some(v) => v,
            None => return Resp3Value::Error("ERR longitude must be a float".into()),
        };
        let lat = match arg_f64(args, pos + 1) {
            Some(v) => v,
            None => return Resp3Value::Error("ERR latitude must be a float".into()),
        };
        let member = match arg_bytes(args, pos + 2) {
            Some(m) => m,
            None => return Resp3Value::Error("ERR member required".into()),
        };
        locations.push((lat, lon, member));
        pos += 3;
    }
    match state.geospatial_store.geoadd(&key, locations, nx, xx, ch) {
        Ok(added) => Resp3Value::Integer(added as i64),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `GEOPOS key member [member ...]`
pub(super) async fn cmd_geopos(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("GEOPOS");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let members: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    match state.geospatial_store.geopos(&key, &members) {
        Ok(positions) => Resp3Value::Array(
            positions
                .into_iter()
                .map(|pos| match pos {
                    // Redis convention: longitude first, latitude second
                    Some(c) => Resp3Value::Array(vec![
                        Resp3Value::BulkString(format!("{:.6}", c.lon).into_bytes()),
                        Resp3Value::BulkString(format!("{:.6}", c.lat).into_bytes()),
                    ]),
                    None => Resp3Value::Null,
                })
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `GEODIST key member1 member2 [m|km|mi|ft]`
pub(super) async fn cmd_geodist(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 4 {
        return err_wrong_args("GEODIST");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let m1 = match arg_bytes(args, 2) {
        Some(m) => m,
        None => return Resp3Value::Error("ERR member1 required".into()),
    };
    let m2 = match arg_bytes(args, 3) {
        Some(m) => m,
        None => return Resp3Value::Error("ERR member2 required".into()),
    };
    let unit: DistanceUnit = args
        .get(4)
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(DistanceUnit::Meters);
    match state.geospatial_store.geodist(&key, &m1, &m2, unit) {
        Ok(Some(dist)) => Resp3Value::BulkString(format!("{dist:.4}").into_bytes()),
        Ok(None) => Resp3Value::Null,
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `GEOHASH key member [member ...]`
pub(super) async fn cmd_geohash(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 3 {
        return err_wrong_args("GEOHASH");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let members: Vec<Vec<u8>> = (2..args.len()).filter_map(|i| arg_bytes(args, i)).collect();
    match state.geospatial_store.geohash(&key, &members) {
        Ok(hashes) => Resp3Value::Array(
            hashes
                .into_iter()
                .map(|h| match h {
                    Some(s) => Resp3Value::BulkString(s.into_bytes()),
                    None => Resp3Value::Null,
                })
                .collect(),
        ),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `GEORADIUS key longitude latitude radius unit [WITHCOORD] [WITHDIST] [COUNT n] [ASC|DESC]`
#[allow(clippy::too_many_lines)]
pub(super) async fn cmd_georadius(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 6 {
        return err_wrong_args("GEORADIUS");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    // Redis convention: longitude first, latitude second
    let lon = match arg_f64(args, 2) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR longitude must be a float".into()),
    };
    let lat = match arg_f64(args, 3) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR latitude must be a float".into()),
    };
    let radius = match arg_f64(args, 4) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR radius must be a float".into()),
    };
    let unit: DistanceUnit = args
        .get(5)
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(DistanceUnit::Meters);
    let mut with_dist = false;
    let mut with_coord = false;
    let mut count: Option<usize> = None;
    let mut sort: Option<String> = None;
    let mut i = 6;
    while i < args.len() {
        match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("WITHDIST") => {
                with_dist = true;
                i += 1;
            }
            Some("WITHCOORD") => {
                with_coord = true;
                i += 1;
            }
            Some("COUNT") => {
                count = arg_u64(args, i + 1).map(|n| n as usize);
                i += 2;
            }
            Some("ASC") => {
                sort = Some("ASC".into());
                i += 1;
            }
            Some("DESC") => {
                sort = Some("DESC".into());
                i += 1;
            }
            _ => i += 1,
        }
    }
    // Internal API uses (lat, lon) — convert from Redis (lon, lat)
    match state.geospatial_store.georadius(
        &key,
        lat,
        lon,
        radius,
        unit,
        with_dist,
        with_coord,
        count,
        sort.as_deref(),
    ) {
        Ok(results) => geo_results_to_resp3(results),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `GEORADIUSBYMEMBER key member radius unit [WITHCOORD] [WITHDIST] [COUNT n] [ASC|DESC]`
pub(super) async fn cmd_georadiusbymember(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 5 {
        return err_wrong_args("GEORADIUSBYMEMBER");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let member = match arg_bytes(args, 2) {
        Some(m) => m,
        None => return Resp3Value::Error("ERR member required".into()),
    };
    let radius = match arg_f64(args, 3) {
        Some(v) => v,
        None => return Resp3Value::Error("ERR radius must be a float".into()),
    };
    let unit: DistanceUnit = args
        .get(4)
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(DistanceUnit::Meters);
    let mut with_dist = false;
    let mut with_coord = false;
    let mut count: Option<usize> = None;
    let mut sort: Option<String> = None;
    let mut i = 5;
    while i < args.len() {
        match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("WITHDIST") => {
                with_dist = true;
                i += 1;
            }
            Some("WITHCOORD") => {
                with_coord = true;
                i += 1;
            }
            Some("COUNT") => {
                count = arg_u64(args, i + 1).map(|n| n as usize);
                i += 2;
            }
            Some("ASC") => {
                sort = Some("ASC".into());
                i += 1;
            }
            Some("DESC") => {
                sort = Some("DESC".into());
                i += 1;
            }
            _ => i += 1,
        }
    }
    match state.geospatial_store.georadiusbymember(
        &key,
        &member,
        radius,
        unit,
        with_dist,
        with_coord,
        count,
        sort.as_deref(),
    ) {
        Ok(results) => geo_results_to_resp3(results),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

/// `GEOSEARCH key FROMMEMBER member|FROMLONLAT lon lat BYRADIUS r unit|BYBOX w h unit …`
#[allow(clippy::too_many_lines)]
pub(super) async fn cmd_geosearch(state: &AppState, args: &[Resp3Value]) -> Resp3Value {
    if args.len() < 6 {
        return err_wrong_args("GEOSEARCH");
    }
    let key = match arg_str(args, 1) {
        Some(k) => k,
        None => return Resp3Value::Error("ERR key must be a string".into()),
    };
    let mut from_member: Option<Vec<u8>> = None;
    let mut from_lonlat: Option<(f64, f64)> = None;
    let mut by_radius: Option<(f64, DistanceUnit)> = None;
    let mut by_box: Option<(f64, f64, DistanceUnit)> = None;
    let mut with_dist = false;
    let mut with_coord = false;
    let mut count: Option<usize> = None;
    let mut sort: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("FROMMEMBER") => {
                from_member = arg_bytes(args, i + 1);
                i += 2;
            }
            Some("FROMLONLAT") => {
                let lon = arg_f64(args, i + 1).unwrap_or(0.0);
                let lat = arg_f64(args, i + 2).unwrap_or(0.0);
                from_lonlat = Some((lon, lat));
                i += 3;
            }
            Some("BYRADIUS") => {
                let r = arg_f64(args, i + 1).unwrap_or(0.0);
                let u = args
                    .get(i + 2)
                    .and_then(|a| a.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(DistanceUnit::Meters);
                by_radius = Some((r, u));
                i += 3;
            }
            Some("BYBOX") => {
                let w = arg_f64(args, i + 1).unwrap_or(0.0);
                let h = arg_f64(args, i + 2).unwrap_or(0.0);
                let u = args
                    .get(i + 3)
                    .and_then(|a| a.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(DistanceUnit::Meters);
                by_box = Some((w, h, u));
                i += 4;
            }
            Some("WITHDIST") => {
                with_dist = true;
                i += 1;
            }
            Some("WITHCOORD") => {
                with_coord = true;
                i += 1;
            }
            Some("COUNT") => {
                count = arg_u64(args, i + 1).map(|n| n as usize);
                i += 2;
            }
            Some("ASC") => {
                sort = Some("ASC".into());
                i += 1;
            }
            Some("DESC") => {
                sort = Some("DESC".into());
                i += 1;
            }
            _ => i += 1,
        }
    }
    match state.geospatial_store.geosearch(
        &key,
        from_member.as_deref(),
        from_lonlat,
        by_radius,
        by_box,
        with_dist,
        with_coord,
        false,
        count,
        sort.as_deref(),
    ) {
        Ok(results) => geo_results_to_resp3(results),
        Err(e) => Resp3Value::Error(format!("ERR {e}")),
    }
}

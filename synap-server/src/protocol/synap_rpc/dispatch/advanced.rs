use super::{AppState, SynapValue, arg_bytes, arg_float, arg_int, arg_str};

/// Serialize a `Vec<GeospatialRadiusResult>` — `(member, dist?, coord?)` — to a
/// `SynapValue::Array`.  Each element is an Array:
///   - `[member_bytes]` if neither dist nor coord are present
///   - `[member_bytes, dist_float, [lat, lon]]` when both are requested;
///     absent optional fields are replaced by `Null`.
fn geo_results_to_value(
    results: Vec<crate::core::geospatial::GeospatialRadiusResult>,
) -> SynapValue {
    SynapValue::Array(
        results
            .into_iter()
            .map(|(member, dist, coord)| {
                if dist.is_none() && coord.is_none() {
                    SynapValue::Bytes(member)
                } else {
                    SynapValue::Array(vec![
                        SynapValue::Bytes(member),
                        dist.map(SynapValue::Float).unwrap_or(SynapValue::Null),
                        coord
                            .map(|c| {
                                SynapValue::Array(vec![
                                    SynapValue::Float(c.lat),
                                    SynapValue::Float(c.lon),
                                ])
                            })
                            .unwrap_or(SynapValue::Null),
                    ])
                }
            })
            .collect(),
    )
}

pub(super) async fn run(
    state: &AppState,
    command: &str,
    args: &[SynapValue],
) -> Result<SynapValue, String> {
    match command {
        // ── Geospatial ────────────────────────────────────────────────────────
        "GEOADD" => {
            // GEOADD key [NX] [XX] [CH] lat lon member [lat lon member ...]
            // Internal ordering: (lat, lon, member)
            let key = arg_str(args, 0)?;
            let mut nx = false;
            let mut xx = false;
            let mut ch = false;
            let mut pos = 1usize;
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
            if (args.len() - pos) % 3 != 0 {
                return Err(
                    "ERR wrong number of arguments for 'GEOADD': expected lat/lon/member triplets"
                        .into(),
                );
            }
            let mut locations = Vec::new();
            while pos + 2 < args.len() {
                let lat = arg_float(args, pos)?;
                let lon = arg_float(args, pos + 1)?;
                let member = arg_bytes(args, pos + 2)?;
                locations.push((lat, lon, member));
                pos += 3;
            }
            state
                .geospatial_store
                .geoadd(&key, locations, nx, xx, ch)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "GEOPOS" => {
            // GEOPOS key member [member ...]
            let key = arg_str(args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            state
                .geospatial_store
                .geopos(&key, &members)
                .map(|coords| {
                    SynapValue::Array(
                        coords
                            .into_iter()
                            .map(|opt| match opt {
                                None => SynapValue::Null,
                                Some(c) => SynapValue::Array(vec![
                                    SynapValue::Float(c.lat),
                                    SynapValue::Float(c.lon),
                                ]),
                            })
                            .collect(),
                    )
                })
                .map_err(|e| e.to_string())
        }
        "GEODIST" => {
            // GEODIST key member1 member2 [unit]
            let key = arg_str(args, 0)?;
            let member1 = arg_bytes(args, 1)?;
            let member2 = arg_bytes(args, 2)?;
            let unit = args
                .get(3)
                .and_then(|v| v.as_str())
                .unwrap_or("m")
                .parse::<crate::core::geospatial::DistanceUnit>()
                .map_err(|e| e.to_string())?;
            state
                .geospatial_store
                .geodist(&key, &member1, &member2, unit)
                .map(|opt| opt.map(SynapValue::Float).unwrap_or(SynapValue::Null))
                .map_err(|e| e.to_string())
        }
        "GEOHASH" => {
            // GEOHASH key member [member ...]
            let key = arg_str(args, 0)?;
            let members: Vec<Vec<u8>> = args[1..]
                .iter()
                .filter_map(|v| v.as_bytes().map(|b| b.to_vec()))
                .collect();
            state
                .geospatial_store
                .geohash(&key, &members)
                .map(|hashes| {
                    SynapValue::Array(
                        hashes
                            .into_iter()
                            .map(|opt| match opt {
                                None => SynapValue::Null,
                                Some(h) => SynapValue::Str(h),
                            })
                            .collect(),
                    )
                })
                .map_err(|e| e.to_string())
        }
        "GEORADIUS" => {
            // GEORADIUS key lat lon radius unit [WITHCOORD] [WITHDIST] [COUNT n] [ASC|DESC]
            let key = arg_str(args, 0)?;
            let lat = arg_float(args, 1)?;
            let lon = arg_float(args, 2)?;
            let radius = arg_float(args, 3)?;
            let unit = arg_str(args, 4)?
                .parse::<crate::core::geospatial::DistanceUnit>()
                .map_err(|e| e.to_string())?;
            let mut with_coord = false;
            let mut with_dist = false;
            let mut count: Option<usize> = None;
            let mut sort: Option<String> = None;
            let mut i = 5;
            while i < args.len() {
                match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
                    Some("WITHCOORD") => {
                        with_coord = true;
                        i += 1;
                    }
                    Some("WITHDIST") => {
                        with_dist = true;
                        i += 1;
                    }
                    Some("COUNT") => {
                        i += 1;
                        count = Some(arg_int(args, i)? as usize);
                        i += 1;
                    }
                    Some("ASC") | Some("DESC") => {
                        sort = args[i].as_str().map(|s| s.to_ascii_uppercase());
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            state
                .geospatial_store
                .georadius(
                    &key,
                    lat,
                    lon,
                    radius,
                    unit,
                    with_dist,
                    with_coord,
                    count,
                    sort.as_deref(),
                )
                .map(geo_results_to_value)
                .map_err(|e| e.to_string())
        }
        "GEORADIUSBYMEMBER" => {
            // GEORADIUSBYMEMBER key member radius unit [WITHCOORD] [WITHDIST] [COUNT n] [ASC|DESC]
            let key = arg_str(args, 0)?;
            let member = arg_bytes(args, 1)?;
            let radius = arg_float(args, 2)?;
            let unit = arg_str(args, 3)?
                .parse::<crate::core::geospatial::DistanceUnit>()
                .map_err(|e| e.to_string())?;
            let mut with_coord = false;
            let mut with_dist = false;
            let mut count: Option<usize> = None;
            let mut sort: Option<String> = None;
            let mut i = 4;
            while i < args.len() {
                match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
                    Some("WITHCOORD") => {
                        with_coord = true;
                        i += 1;
                    }
                    Some("WITHDIST") => {
                        with_dist = true;
                        i += 1;
                    }
                    Some("COUNT") => {
                        i += 1;
                        count = Some(arg_int(args, i)? as usize);
                        i += 1;
                    }
                    Some("ASC") | Some("DESC") => {
                        sort = args[i].as_str().map(|s| s.to_ascii_uppercase());
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            state
                .geospatial_store
                .georadiusbymember(
                    &key,
                    &member,
                    radius,
                    unit,
                    with_dist,
                    with_coord,
                    count,
                    sort.as_deref(),
                )
                .map(geo_results_to_value)
                .map_err(|e| e.to_string())
        }
        "GEOSEARCH" => {
            // GEOSEARCH key FROMMEMBER member | FROMLONLAT lon lat
            //           BYRADIUS r unit | BYBOX w h unit
            //           [WITHCOORD] [WITHDIST] [COUNT n] [ASC|DESC]
            let key = arg_str(args, 0)?;
            let mut from_member: Option<Vec<u8>> = None;
            let mut from_lonlat: Option<(f64, f64)> = None;
            let mut by_radius: Option<(f64, crate::core::geospatial::DistanceUnit)> = None;
            let mut by_box: Option<(f64, f64, crate::core::geospatial::DistanceUnit)> = None;
            let mut with_coord = false;
            let mut with_dist = false;
            let mut count: Option<usize> = None;
            let mut sort: Option<String> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str().map(|s| s.to_ascii_uppercase()).as_deref() {
                    Some("FROMMEMBER") => {
                        i += 1;
                        from_member = Some(arg_bytes(args, i)?);
                        i += 1;
                    }
                    Some("FROMLONLAT") => {
                        i += 1;
                        let lon = arg_float(args, i)?;
                        i += 1;
                        let lat = arg_float(args, i)?;
                        from_lonlat = Some((lon, lat));
                        i += 1;
                    }
                    Some("BYRADIUS") => {
                        i += 1;
                        let r = arg_float(args, i)?;
                        i += 1;
                        let u = arg_str(args, i)?
                            .parse::<crate::core::geospatial::DistanceUnit>()
                            .map_err(|e| e.to_string())?;
                        by_radius = Some((r, u));
                        i += 1;
                    }
                    Some("BYBOX") => {
                        i += 1;
                        let w = arg_float(args, i)?;
                        i += 1;
                        let h = arg_float(args, i)?;
                        i += 1;
                        let u = arg_str(args, i)?
                            .parse::<crate::core::geospatial::DistanceUnit>()
                            .map_err(|e| e.to_string())?;
                        by_box = Some((w, h, u));
                        i += 1;
                    }
                    Some("WITHCOORD") => {
                        with_coord = true;
                        i += 1;
                    }
                    Some("WITHDIST") => {
                        with_dist = true;
                        i += 1;
                    }
                    Some("COUNT") => {
                        i += 1;
                        count = Some(arg_int(args, i)? as usize);
                        i += 1;
                    }
                    Some("ASC") | Some("DESC") => {
                        sort = args[i].as_str().map(|s| s.to_ascii_uppercase());
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            state
                .geospatial_store
                .geosearch(
                    &key,
                    from_member.as_deref(),
                    from_lonlat,
                    by_radius,
                    by_box,
                    with_dist,
                    with_coord,
                    false, // with_hash not yet supported in the store
                    count,
                    sort.as_deref(),
                )
                .map(geo_results_to_value)
                .map_err(|e| e.to_string())
        }
        "GEOSTATS" => {
            let s = state.geospatial_store.stats();
            Ok(SynapValue::Map(vec![
                (
                    SynapValue::Str("total_keys".into()),
                    SynapValue::Int(s.total_keys as i64),
                ),
                (
                    SynapValue::Str("total_locations".into()),
                    SynapValue::Int(s.total_locations as i64),
                ),
                (
                    SynapValue::Str("geoadd_count".into()),
                    SynapValue::Int(s.geoadd_count as i64),
                ),
                (
                    SynapValue::Str("geodist_count".into()),
                    SynapValue::Int(s.geodist_count as i64),
                ),
                (
                    SynapValue::Str("georadius_count".into()),
                    SynapValue::Int(s.georadius_count as i64),
                ),
                (
                    SynapValue::Str("geopos_count".into()),
                    SynapValue::Int(s.geopos_count as i64),
                ),
                (
                    SynapValue::Str("geohash_count".into()),
                    SynapValue::Int(s.geohash_count as i64),
                ),
            ]))
        }

        // ── Queue ─────────────────────────────────────────────────────────────
        "QCREATE" => {
            let name = arg_str(args, 0)?;
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.create_queue(&name, None)
                .await
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "QDELETE" => {
            let name = arg_str(args, 0)?;
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.delete_queue(&name)
                .await
                .map(SynapValue::Bool)
                .map_err(|e| e.to_string())
        }
        "QLIST" => {
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.list_queues()
                .await
                .map(|names| SynapValue::Array(names.into_iter().map(SynapValue::Str).collect()))
                .map_err(|e| e.to_string())
        }
        "QPUBLISH" => {
            // QPUBLISH queue payload [priority] [max_retries]
            let name = arg_str(args, 0)?;
            let payload = arg_bytes(args, 1)?;
            let priority = args.get(2).and_then(|v| v.as_int()).map(|n| n as u8);
            let max_retries = args.get(3).and_then(|v| v.as_int()).map(|n| n as u32);
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.publish(&name, payload, priority, max_retries)
                .await
                .map(SynapValue::Str)
                .map_err(|e| e.to_string())
        }
        "QCONSUME" => {
            // QCONSUME queue consumer_id
            let name = arg_str(args, 0)?;
            let consumer_id = arg_str(args, 1)?;
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.consume(&name, &consumer_id)
                .await
                .map(|opt| match opt {
                    None => SynapValue::Null,
                    Some(msg) => SynapValue::Map(vec![
                        (
                            SynapValue::Str("id".into()),
                            SynapValue::Str(msg.id.clone()),
                        ),
                        (
                            SynapValue::Str("payload".into()),
                            SynapValue::Bytes((*msg.payload).clone()),
                        ),
                        (
                            SynapValue::Str("priority".into()),
                            SynapValue::Int(msg.priority as i64),
                        ),
                        (
                            SynapValue::Str("retry_count".into()),
                            SynapValue::Int(msg.retry_count as i64),
                        ),
                    ]),
                })
                .map_err(|e| e.to_string())
        }
        "QACK" => {
            let name = arg_str(args, 0)?;
            let message_id = arg_str(args, 1)?;
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.ack(&name, &message_id)
                .await
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "QNACK" => {
            // QNACK queue message_id [requeue:bool]  (default requeue = true)
            let name = arg_str(args, 0)?;
            let message_id = arg_str(args, 1)?;
            let requeue = args
                .get(2)
                .and_then(|v| match v {
                    SynapValue::Bool(b) => Some(*b),
                    SynapValue::Int(n) => Some(*n != 0),
                    _ => None,
                })
                .unwrap_or(true);
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.nack(&name, &message_id, requeue)
                .await
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "QSTATS" => {
            let name = arg_str(args, 0)?;
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.stats(&name)
                .await
                .map(|s| {
                    SynapValue::Map(vec![
                        (
                            SynapValue::Str("depth".into()),
                            SynapValue::Int(s.depth as i64),
                        ),
                        (
                            SynapValue::Str("consumers".into()),
                            SynapValue::Int(s.consumers as i64),
                        ),
                        (
                            SynapValue::Str("published".into()),
                            SynapValue::Int(s.published as i64),
                        ),
                        (
                            SynapValue::Str("consumed".into()),
                            SynapValue::Int(s.consumed as i64),
                        ),
                        (
                            SynapValue::Str("acked".into()),
                            SynapValue::Int(s.acked as i64),
                        ),
                        (
                            SynapValue::Str("nacked".into()),
                            SynapValue::Int(s.nacked as i64),
                        ),
                        (
                            SynapValue::Str("dead_lettered".into()),
                            SynapValue::Int(s.dead_lettered as i64),
                        ),
                    ])
                })
                .map_err(|e| e.to_string())
        }
        "QPURGE" => {
            let name = arg_str(args, 0)?;
            let qm = state
                .queue_manager
                .as_deref()
                .ok_or_else(|| "ERR queue subsystem not enabled".to_string())?;
            qm.purge(&name)
                .await
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }

        // ── Stream ────────────────────────────────────────────────────────────
        "SCREATE" => {
            let room = arg_str(args, 0)?;
            let sm = state
                .stream_manager
                .as_deref()
                .ok_or_else(|| "ERR stream subsystem not enabled".to_string())?;
            sm.create_room(&room)
                .await
                .map(|()| SynapValue::Str("OK".into()))
        }
        "SPUBLISH" => {
            // SPUBLISH room event_type data
            let room = arg_str(args, 0)?;
            let event_type = arg_str(args, 1)?;
            let data = arg_bytes(args, 2)?;
            let sm = state
                .stream_manager
                .as_deref()
                .ok_or_else(|| "ERR stream subsystem not enabled".to_string())?;
            sm.publish(&room, &event_type, data)
                .await
                .map(|offset| SynapValue::Int(offset as i64))
        }
        "SREAD" => {
            // SREAD room subscriber_id from_offset [limit]
            let room = arg_str(args, 0)?;
            let subscriber_id = arg_str(args, 1)?;
            let from_offset = arg_int(args, 2)? as u64;
            let limit = args
                .get(3)
                .and_then(|v| v.as_int())
                .map(|n| n as usize)
                .unwrap_or(100);
            let sm = state
                .stream_manager
                .as_deref()
                .ok_or_else(|| "ERR stream subsystem not enabled".to_string())?;
            sm.consume(&room, &subscriber_id, from_offset, limit)
                .await
                .map(|events| {
                    SynapValue::Array(
                        events
                            .into_iter()
                            .map(|e| {
                                SynapValue::Map(vec![
                                    (SynapValue::Str("id".into()), SynapValue::Str(e.id)),
                                    (
                                        SynapValue::Str("offset".into()),
                                        SynapValue::Int(e.offset as i64),
                                    ),
                                    (SynapValue::Str("event".into()), SynapValue::Str(e.event)),
                                    (SynapValue::Str("data".into()), SynapValue::Bytes(e.data)),
                                    (
                                        SynapValue::Str("timestamp".into()),
                                        SynapValue::Int(e.timestamp as i64),
                                    ),
                                ])
                            })
                            .collect(),
                    )
                })
        }
        "SDELETE" => {
            let room = arg_str(args, 0)?;
            let sm = state
                .stream_manager
                .as_deref()
                .ok_or_else(|| "ERR stream subsystem not enabled".to_string())?;
            sm.delete_room(&room)
                .await
                .map(|()| SynapValue::Str("OK".into()))
        }
        "SLIST" => {
            let sm = state
                .stream_manager
                .as_deref()
                .ok_or_else(|| "ERR stream subsystem not enabled".to_string())?;
            let rooms = sm.list_rooms().await;
            Ok(SynapValue::Array(
                rooms.into_iter().map(SynapValue::Str).collect(),
            ))
        }
        "SSTATS" => {
            let room = arg_str(args, 0)?;
            let sm = state
                .stream_manager
                .as_deref()
                .ok_or_else(|| "ERR stream subsystem not enabled".to_string())?;
            sm.room_stats(&room).await.map(|s| {
                SynapValue::Map(vec![
                    (SynapValue::Str("name".into()), SynapValue::Str(s.name)),
                    (
                        SynapValue::Str("message_count".into()),
                        SynapValue::Int(s.message_count as i64),
                    ),
                    (
                        SynapValue::Str("min_offset".into()),
                        SynapValue::Int(s.min_offset as i64),
                    ),
                    (
                        SynapValue::Str("max_offset".into()),
                        SynapValue::Int(s.max_offset as i64),
                    ),
                    (
                        SynapValue::Str("subscriber_count".into()),
                        SynapValue::Int(s.subscriber_count as i64),
                    ),
                    (
                        SynapValue::Str("total_published".into()),
                        SynapValue::Int(s.total_published as i64),
                    ),
                ])
            })
        }

        // ── Pub/Sub ───────────────────────────────────────────────────────────
        "PUBLISH" => {
            // PUBLISH topic payload_bytes
            // payload is deserialized as JSON when possible; falls back to string
            let topic = arg_str(args, 0)?;
            let payload_bytes = arg_bytes(args, 1)?;
            let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes)
                .unwrap_or_else(|_| {
                    serde_json::Value::String(String::from_utf8_lossy(&payload_bytes).into_owned())
                });
            let ps = state
                .pubsub_router
                .as_deref()
                .ok_or_else(|| "ERR pubsub subsystem not enabled".to_string())?;
            ps.publish(&topic, payload_json, None)
                .map(|r| {
                    SynapValue::Map(vec![
                        (
                            SynapValue::Str("message_id".into()),
                            SynapValue::Str(r.message_id),
                        ),
                        (
                            SynapValue::Str("subscribers_matched".into()),
                            SynapValue::Int(r.subscribers_matched as i64),
                        ),
                    ])
                })
                .map_err(|e| e.to_string())
        }
        "SUBSCRIBE" => {
            // SUBSCRIBE topic [topic ...]
            // Returns subscriber_id; push frames arrive via the SynapRPC connection layer
            let topics: Vec<String> = args
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            if topics.is_empty() {
                return Err("ERR SUBSCRIBE requires at least one topic".into());
            }
            let ps = state
                .pubsub_router
                .as_deref()
                .ok_or_else(|| "ERR pubsub subsystem not enabled".to_string())?;
            ps.subscribe(topics)
                .map(|r| {
                    SynapValue::Map(vec![
                        (
                            SynapValue::Str("subscriber_id".into()),
                            SynapValue::Str(r.subscriber_id),
                        ),
                        (
                            SynapValue::Str("subscription_count".into()),
                            SynapValue::Int(r.subscription_count as i64),
                        ),
                    ])
                })
                .map_err(|e| e.to_string())
        }
        "UNSUBSCRIBE" => {
            // UNSUBSCRIBE subscriber_id [topic ...]  (no topics = unsubscribe all)
            let subscriber_id = arg_str(args, 0)?;
            let topics: Vec<String> = args[1..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            let topics_opt = if topics.is_empty() {
                None
            } else {
                Some(topics)
            };
            let ps = state
                .pubsub_router
                .as_deref()
                .ok_or_else(|| "ERR pubsub subsystem not enabled".to_string())?;
            ps.unsubscribe(&subscriber_id, topics_opt)
                .map(|n| SynapValue::Int(n as i64))
                .map_err(|e| e.to_string())
        }
        "TOPICS" => {
            let ps = state
                .pubsub_router
                .as_deref()
                .ok_or_else(|| "ERR pubsub subsystem not enabled".to_string())?;
            Ok(SynapValue::Array(
                ps.list_topics().into_iter().map(SynapValue::Str).collect(),
            ))
        }
        "PSSTATS" => {
            let ps = state
                .pubsub_router
                .as_deref()
                .ok_or_else(|| "ERR pubsub subsystem not enabled".to_string())?;
            let s = ps.get_stats();
            Ok(SynapValue::Map(vec![
                (
                    SynapValue::Str("total_topics".into()),
                    SynapValue::Int(s.total_topics as i64),
                ),
                (
                    SynapValue::Str("total_subscribers".into()),
                    SynapValue::Int(s.total_subscribers as i64),
                ),
                (
                    SynapValue::Str("total_wildcard_subscriptions".into()),
                    SynapValue::Int(s.total_wildcard_subscriptions as i64),
                ),
                (
                    SynapValue::Str("messages_published".into()),
                    SynapValue::Int(s.messages_published as i64),
                ),
                (
                    SynapValue::Str("messages_delivered".into()),
                    SynapValue::Int(s.messages_delivered as i64),
                ),
            ]))
        }

        // ── Transactions ──────────────────────────────────────────────────────
        "MULTI" => {
            // MULTI client_id
            let client_id = arg_str(args, 0)?;
            state
                .transaction_manager
                .multi(client_id)
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "EXEC" => {
            // EXEC client_id
            let client_id = arg_str(args, 0)?;
            state
                .transaction_manager
                .exec(&client_id)
                .await
                .map(|opt| match opt {
                    None => SynapValue::Null, // watched key changed — transaction aborted
                    Some(results) => SynapValue::Array(
                        results
                            .into_iter()
                            .map(|v| SynapValue::Str(serde_json::to_string(&v).unwrap_or_default()))
                            .collect(),
                    ),
                })
                .map_err(|e| e.to_string())
        }
        "DISCARD" => {
            // DISCARD client_id
            let client_id = arg_str(args, 0)?;
            state
                .transaction_manager
                .discard(&client_id)
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "WATCH" => {
            // WATCH client_id key [key ...]
            let client_id = arg_str(args, 0)?;
            let keys: Vec<String> = args[1..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            if keys.is_empty() {
                return Err("ERR WATCH requires at least one key".into());
            }
            state
                .transaction_manager
                .watch(&client_id, keys)
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }
        "UNWATCH" => {
            // UNWATCH client_id
            let client_id = arg_str(args, 0)?;
            state
                .transaction_manager
                .unwatch(&client_id)
                .map(|()| SynapValue::Str("OK".into()))
                .map_err(|e| e.to_string())
        }

        // ── Scripting ─────────────────────────────────────────────────────────
        "EVAL" => {
            // EVAL script numkeys [key ...] [arg ...]
            let script = arg_str(args, 0)?;
            let numkeys = arg_int(args, 1)? as usize;
            let key_end = (2 + numkeys).min(args.len());
            let keys: Vec<String> = args[2..key_end]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            let script_args: Vec<String> = args[key_end..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            let ctx = crate::scripting::ScriptExecContext {
                kv_store: state.kv_store.clone(),
                hash_store: state.hash_store.clone(),
                list_store: state.list_store.clone(),
                set_store: state.set_store.clone(),
                sorted_set_store: state.sorted_set_store.clone(),
            };
            state
                .script_manager
                .eval(ctx, &script, keys, script_args, None)
                .await
                .map(|(v, _sha)| SynapValue::Str(serde_json::to_string(&v).unwrap_or_default()))
                .map_err(|e| e.to_string())
        }
        "EVALSHA" => {
            // EVALSHA sha numkeys [key ...] [arg ...]
            let sha = arg_str(args, 0)?;
            let numkeys = arg_int(args, 1)? as usize;
            let key_end = (2 + numkeys).min(args.len());
            let keys: Vec<String> = args[2..key_end]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            let script_args: Vec<String> = args[key_end..]
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            let ctx = crate::scripting::ScriptExecContext {
                kv_store: state.kv_store.clone(),
                hash_store: state.hash_store.clone(),
                list_store: state.list_store.clone(),
                set_store: state.set_store.clone(),
                sorted_set_store: state.sorted_set_store.clone(),
            };
            state
                .script_manager
                .evalsha(ctx, &sha, keys, script_args, None)
                .await
                .map(|v| SynapValue::Str(serde_json::to_string(&v).unwrap_or_default()))
                .map_err(|e| e.to_string())
        }
        "SCRIPT.LOAD" => {
            let source = arg_str(args, 0)?;
            let sha = state.script_manager.load_script(&source);
            Ok(SynapValue::Str(sha))
        }
        "SCRIPT.EXISTS" => {
            let hashes: Vec<String> = args
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect();
            let exists = state.script_manager.script_exists(&hashes);
            Ok(SynapValue::Array(
                exists.into_iter().map(SynapValue::Bool).collect(),
            ))
        }
        "SCRIPT.FLUSH" => {
            let n = state.script_manager.flush();
            Ok(SynapValue::Int(n as i64))
        }
        "SCRIPT.KILL" => {
            let killed = state.script_manager.kill_running();
            Ok(SynapValue::Bool(killed))
        }

        _ => Err(format!("ERR unknown command '{command}'")),
    }
}

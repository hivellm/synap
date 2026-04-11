/**
 * Synap TypeScript SDK - Command Mapper
 *
 * Maps dotted SDK command names + JSON payloads to native wire command names
 * and ordered argument lists for the SynapRPC / RESP3 transports.
 *
 * Commands without a mapping return `null`; the caller should surface an
 * UnsupportedCommandError rather than silently falling back to HTTP.
 *
 * Response shapes are normalised by `mapResponse` so that every SDK manager
 * receives the same JSON structure regardless of the active transport.
 */

// ── mapCommand ─────────────────────────────────────────────────────────────────

/** Result of a successful command mapping. */
export interface MappedCommand {
  /** Wire-level command (e.g. `"SET"`, `"HGET"`). */
  rawCmd: string;
  /** Ordered argument list (values will be stringified or WireValue-encoded). */
  args: unknown[];
}

/**
 * Map a dotted SDK command + JSON payload to a native wire command.
 *
 * @returns `MappedCommand` on success, or `null` if the command has no native mapping.
 */
export function mapCommand(
  cmd: string,
  payload: Record<string, unknown>,
): MappedCommand | null {
  /** Coerce `payload[key]` to a string (empty string if absent). */
  const s = (key: string): string => String(payload[key] ?? '');
  /** Coerce `payload[key]` to a string number with `def` as fallback. */
  const n = (key: string, def: number): string => String(payload[key] ?? def);

  switch (cmd) {
    // ── KV ────────────────────────────────────────────────────────────────────
    case 'kv.get':
      return { rawCmd: 'GET', args: [s('key')] };

    case 'kv.set': {
      const args: unknown[] = [s('key'), payload['value'] ?? ''];
      if (payload['ttl'] != null) args.push('EX', String(payload['ttl']));
      return { rawCmd: 'SET', args };
    }

    case 'kv.del':    return { rawCmd: 'DEL',    args: [s('key')] };
    case 'kv.exists': return { rawCmd: 'EXISTS', args: [s('key')] };
    case 'kv.incr':   return { rawCmd: 'INCR',   args: [s('key')] };
    case 'kv.decr':   return { rawCmd: 'DECR',   args: [s('key')] };

    case 'kv.keys': {
      const prefix = String(payload['prefix'] ?? '');
      return { rawCmd: 'KEYS', args: [prefix ? `${prefix}*` : '*'] };
    }

    case 'kv.expire': return { rawCmd: 'EXPIRE', args: [s('key'), n('ttl', 0)] };
    case 'kv.ttl':    return { rawCmd: 'TTL',    args: [s('key')] };

    // ── Hash ──────────────────────────────────────────────────────────────────
    case 'hash.set':         return { rawCmd: 'HSET',      args: [s('key'), s('field'), s('value')] };
    case 'hash.get':         return { rawCmd: 'HGET',      args: [s('key'), s('field')] };
    case 'hash.getall':      return { rawCmd: 'HGETALL',   args: [s('key')] };
    case 'hash.del':         return { rawCmd: 'HDEL',      args: [s('key'), s('field')] };
    case 'hash.exists':      return { rawCmd: 'HEXISTS',   args: [s('key'), s('field')] };
    case 'hash.keys':        return { rawCmd: 'HKEYS',     args: [s('key')] };
    case 'hash.values':      return { rawCmd: 'HVALS',     args: [s('key')] };
    case 'hash.len':         return { rawCmd: 'HLEN',      args: [s('key')] };
    case 'hash.incrby':      return { rawCmd: 'HINCRBY',   args: [s('key'), s('field'), n('increment', 0)] };
    case 'hash.incrbyfloat': return { rawCmd: 'HINCRBYFLOAT', args: [s('key'), s('field'), n('increment', 0)] };
    case 'hash.setnx':       return { rawCmd: 'HSETNX',   args: [s('key'), s('field'), s('value')] };

    case 'hash.mset': {
      const args: unknown[] = [s('key')];
      const fields = payload['fields'];
      if (fields && typeof fields === 'object' && !Array.isArray(fields)) {
        // HashMap format: { fieldName: value, ... }
        for (const [k, v] of Object.entries(fields as Record<string, unknown>)) {
          args.push(k, String(v));
        }
      } else if (Array.isArray(fields)) {
        // Array format: [{ field, value }, ...]
        for (const item of fields as Array<Record<string, unknown>>) {
          args.push(String(item['field'] ?? ''), String(item['value'] ?? ''));
        }
      }
      return { rawCmd: 'HSET', args };
    }

    case 'hash.mget': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['fields'])) {
        args.push(...(payload['fields'] as string[]));
      }
      return { rawCmd: 'HMGET', args };
    }

    // ── List ──────────────────────────────────────────────────────────────────
    case 'list.lpush':
    case 'list.lpushx': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['values'])) args.push(...(payload['values'] as string[]));
      return { rawCmd: cmd === 'list.lpushx' ? 'LPUSHX' : 'LPUSH', args };
    }

    case 'list.rpush':
    case 'list.rpushx': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['values'])) args.push(...(payload['values'] as string[]));
      return { rawCmd: cmd === 'list.rpushx' ? 'RPUSHX' : 'RPUSH', args };
    }

    case 'list.lpop': {
      const args: unknown[] = [s('key')];
      if (payload['count'] != null) args.push(String(payload['count']));
      return { rawCmd: 'LPOP', args };
    }

    case 'list.rpop': {
      const args: unknown[] = [s('key')];
      if (payload['count'] != null) args.push(String(payload['count']));
      return { rawCmd: 'RPOP', args };
    }

    case 'list.range':    return { rawCmd: 'LRANGE',  args: [s('key'), n('start', 0), n('stop', -1)] };
    case 'list.len':      return { rawCmd: 'LLEN',    args: [s('key')] };
    case 'list.index':    return { rawCmd: 'LINDEX',  args: [s('key'), n('index', 0)] };
    case 'list.set':      return { rawCmd: 'LSET',    args: [s('key'), n('index', 0), String(payload['value'] ?? '')] };
    case 'list.trim':     return { rawCmd: 'LTRIM',   args: [s('key'), n('start', 0), n('end', -1)] };
    case 'list.rem':      return { rawCmd: 'LREM',    args: [s('key'), n('count', 0), String(payload['element'] ?? '')] };
    case 'list.rpoplpush': return { rawCmd: 'RPOPLPUSH', args: [s('key'), s('destination')] };
    case 'list.pos':      return { rawCmd: 'LPOS',    args: [s('key'), String(payload['element'] ?? '')] };

    case 'list.insert': {
      const placement = payload['before'] !== false ? 'BEFORE' : 'AFTER';
      return {
        rawCmd: 'LINSERT',
        args: [s('key'), placement, String(payload['pivot'] ?? ''), String(payload['value'] ?? '')],
      };
    }

    // ── Set ───────────────────────────────────────────────────────────────────
    case 'set.add': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) args.push(...(payload['members'] as string[]));
      return { rawCmd: 'SADD', args };
    }

    case 'set.rem': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) args.push(...(payload['members'] as string[]));
      return { rawCmd: 'SREM', args };
    }

    case 'set.ismember':   return { rawCmd: 'SISMEMBER',   args: [s('key'), s('member')] };
    case 'set.members':    return { rawCmd: 'SMEMBERS',    args: [s('key')] };
    case 'set.card':       return { rawCmd: 'SCARD',       args: [s('key')] };
    case 'set.pop':        return { rawCmd: 'SPOP',        args: [s('key'), n('count', 1)] };
    case 'set.randmember': return { rawCmd: 'SRANDMEMBER', args: [s('key'), n('count', 1)] };
    case 'set.move':       return { rawCmd: 'SMOVE',       args: [s('key'), s('destination'), s('member')] };

    case 'set.inter':
    case 'set.union':
    case 'set.diff': {
      const rawCmd = ({ 'set.inter': 'SINTER', 'set.union': 'SUNION', 'set.diff': 'SDIFF' } as Record<string, string>)[cmd]!;
      const args = Array.isArray(payload['keys']) ? [...(payload['keys'] as string[])] : [];
      return { rawCmd, args };
    }

    case 'set.interstore':
    case 'set.unionstore':
    case 'set.diffstore': {
      const rawCmd = ({
        'set.interstore': 'SINTERSTORE',
        'set.unionstore': 'SUNIONSTORE',
        'set.diffstore': 'SDIFFSTORE',
      } as Record<string, string>)[cmd]!;
      const keys = Array.isArray(payload['keys']) ? (payload['keys'] as string[]) : [];
      return { rawCmd, args: [s('destination'), ...keys] };
    }

    // ── Sorted Set ────────────────────────────────────────────────────────────
    case 'sortedset.zadd': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) {
        for (const m of payload['members'] as Array<{ score: number; member: string }>) {
          args.push(String(m.score), m.member);
        }
      } else {
        args.push(String(payload['score'] ?? 0), String(payload['member'] ?? ''));
      }
      return { rawCmd: 'ZADD', args };
    }

    case 'sortedset.zrem': {
      const args: unknown[] = [s('key')];
      if (Array.isArray(payload['members'])) args.push(...(payload['members'] as string[]));
      else args.push(String(payload['member'] ?? ''));
      return { rawCmd: 'ZREM', args };
    }

    case 'sortedset.zscore':   return { rawCmd: 'ZSCORE',   args: [s('key'), String(payload['member'] ?? '')] };
    case 'sortedset.zcard':    return { rawCmd: 'ZCARD',    args: [s('key')] };
    case 'sortedset.zincrby':  return { rawCmd: 'ZINCRBY',  args: [s('key'), String(payload['increment'] ?? 0), String(payload['member'] ?? '')] };
    case 'sortedset.zrank':    return { rawCmd: 'ZRANK',    args: [s('key'), String(payload['member'] ?? '')] };
    case 'sortedset.zrevrank': return { rawCmd: 'ZREVRANK', args: [s('key'), String(payload['member'] ?? '')] };
    case 'sortedset.zcount':   return { rawCmd: 'ZCOUNT',   args: [s('key'), String(payload['min'] ?? '-inf'), String(payload['max'] ?? '+inf')] };

    case 'sortedset.zrange':
    case 'sortedset.zrevrange': {
      const rawCmd = cmd === 'sortedset.zrevrange' ? 'ZREVRANGE' : 'ZRANGE';
      const args: unknown[] = [s('key'), n('start', 0), n('stop', -1)];
      if (payload['withscores']) args.push('WITHSCORES');
      return { rawCmd, args };
    }

    case 'sortedset.zrangebyscore': {
      const args: unknown[] = [s('key'), String(payload['min'] ?? '-inf'), String(payload['max'] ?? '+inf')];
      if (payload['withscores']) args.push('WITHSCORES');
      return { rawCmd: 'ZRANGEBYSCORE', args };
    }

    case 'sortedset.zpopmin':
    case 'sortedset.zpopmax':
      return {
        rawCmd: cmd === 'sortedset.zpopmax' ? 'ZPOPMAX' : 'ZPOPMIN',
        args: [s('key'), n('count', 1)],
      };

    case 'sortedset.zremrangebyrank':  return { rawCmd: 'ZREMRANGEBYRANK',  args: [s('key'), n('start', 0), n('stop', -1)] };
    case 'sortedset.zremrangebyscore': return { rawCmd: 'ZREMRANGEBYSCORE', args: [s('key'), String(payload['min'] ?? '-inf'), String(payload['max'] ?? '+inf')] };

    case 'sortedset.zinterstore':
    case 'sortedset.zunionstore':
    case 'sortedset.zdiffstore': {
      const rawCmd = ({
        'sortedset.zinterstore': 'ZINTERSTORE',
        'sortedset.zunionstore': 'ZUNIONSTORE',
        'sortedset.zdiffstore': 'ZDIFFSTORE',
      } as Record<string, string>)[cmd]!;
      const keys = Array.isArray(payload['keys']) ? (payload['keys'] as string[]) : [];
      return { rawCmd, args: [s('destination'), String(keys.length), ...keys] };
    }

    // ── Queue ─────────────────────────────────────────────────────────────────
    case 'queue.create':
      return {
        rawCmd: 'QCREATE',
        args: [
          s('name'),
          String(payload['max_depth'] ?? 0),
          String(payload['ack_deadline_secs'] ?? 30),
        ],
      };

    case 'queue.delete': return { rawCmd: 'QDELETE', args: [s('queue')] };
    case 'queue.list':   return { rawCmd: 'QLIST',   args: [] };
    case 'queue.purge':  return { rawCmd: 'QPURGE',  args: [s('queue')] };

    case 'queue.publish': {
      const pl = payload['payload'];
      let payloadArg: unknown;
      if (pl instanceof Uint8Array || Buffer.isBuffer(pl as unknown)) {
        payloadArg = pl;
      } else if (typeof pl === 'string') {
        payloadArg = pl;
      } else {
        payloadArg = JSON.stringify(pl ?? '');
      }
      return {
        rawCmd: 'QPUBLISH',
        args: [
          s('queue'),
          payloadArg,
          String(payload['priority'] ?? 0),
          String(payload['max_retries'] ?? 3),
        ],
      };
    }

    case 'queue.consume': return { rawCmd: 'QCONSUME', args: [s('queue'), s('consumer_id')] };
    case 'queue.ack':     return { rawCmd: 'QACK',     args: [s('queue'), s('message_id')] };
    case 'queue.nack':    return { rawCmd: 'QNACK',    args: [s('queue'), s('message_id'), String(payload['requeue'] ?? true)] };
    case 'queue.stats':   return { rawCmd: 'QSTATS',   args: [s('queue')] };

    // ── Stream ────────────────────────────────────────────────────────────────
    case 'stream.create':
      return {
        rawCmd: 'SCREATE',
        args: [s('room'), String(payload['max_events'] ?? 0)],
      };

    case 'stream.delete':  return { rawCmd: 'SDELETE', args: [s('room')] };
    case 'stream.list':    return { rawCmd: 'SLIST',   args: [] };

    case 'stream.publish':
      return {
        rawCmd: 'SPUBLISH',
        args: [s('room'), s('event'), JSON.stringify(payload['data'] ?? {})],
      };

    case 'stream.consume':
      return {
        rawCmd: 'SREAD',
        args: [s('room'), s('subscriber_id'), String(payload['from_offset'] ?? 0)],
      };

    case 'stream.stats': return { rawCmd: 'SSTATS', args: [s('room')] };

    // ── Pub/Sub ───────────────────────────────────────────────────────────────
    case 'pubsub.publish':
      return {
        rawCmd: 'PUBLISH',
        args: [s('topic'), JSON.stringify(payload['payload'] ?? payload['data'] ?? '')],
      };

    case 'pubsub.subscribe': {
      const topics = Array.isArray(payload['topics']) ? (payload['topics'] as string[]) : [];
      return { rawCmd: 'SUBSCRIBE', args: [...topics] };
    }

    case 'pubsub.unsubscribe': {
      const topics = Array.isArray(payload['topics']) ? (payload['topics'] as string[]) : [];
      return { rawCmd: 'UNSUBSCRIBE', args: [s('subscriber_id'), ...topics] };
    }

    case 'pubsub.topics':
    case 'pubsub.list':
      return { rawCmd: 'TOPICS', args: [] };

    // ── Transactions ──────────────────────────────────────────────────────────
    case 'transaction.multi':   return { rawCmd: 'MULTI',   args: [s('client_id')] };
    case 'transaction.exec':    return { rawCmd: 'EXEC',    args: [s('client_id')] };
    case 'transaction.discard': return { rawCmd: 'DISCARD', args: [s('client_id')] };

    case 'transaction.watch': {
      const keys = Array.isArray(payload['keys']) ? (payload['keys'] as string[]) : [];
      return { rawCmd: 'WATCH', args: [s('client_id'), ...keys] };
    }

    case 'transaction.unwatch': return { rawCmd: 'UNWATCH', args: [s('client_id')] };

    // ── Scripts ───────────────────────────────────────────────────────────────
    case 'script.eval': {
      const keys = Array.isArray(payload['keys']) ? (payload['keys'] as string[]) : [];
      const args = Array.isArray(payload['args']) ? (payload['args'] as unknown[]) : [];
      return { rawCmd: 'EVAL', args: [s('script'), String(keys.length), ...keys, ...args.map(String)] };
    }

    case 'script.evalsha': {
      const keys = Array.isArray(payload['keys']) ? (payload['keys'] as string[]) : [];
      const args = Array.isArray(payload['args']) ? (payload['args'] as unknown[]) : [];
      return { rawCmd: 'EVALSHA', args: [s('sha1'), String(keys.length), ...keys, ...args.map(String)] };
    }

    case 'script.load':   return { rawCmd: 'SCRIPT.LOAD',   args: [s('script')] };

    case 'script.exists': {
      const hashes = Array.isArray(payload['hashes']) ? (payload['hashes'] as string[]) : [];
      return { rawCmd: 'SCRIPT.EXISTS', args: [...hashes] };
    }

    case 'script.flush': return { rawCmd: 'SCRIPT.FLUSH', args: [] };
    case 'script.kill':  return { rawCmd: 'SCRIPT.KILL',  args: [] };

    // ── HyperLogLog ───────────────────────────────────────────────────────────
    case 'hyperloglog.pfadd': {
      const elems = Array.isArray(payload['elements']) ? (payload['elements'] as string[]) : [];
      return { rawCmd: 'PFADD', args: [s('key'), ...elems] };
    }

    case 'hyperloglog.pfcount': {
      const keys = Array.isArray(payload['keys']) ? (payload['keys'] as string[]) : [s('key')];
      return { rawCmd: 'PFCOUNT', args: [...keys] };
    }

    case 'hyperloglog.pfmerge': {
      const srcKeys = Array.isArray(payload['source_keys']) ? (payload['source_keys'] as string[]) : [];
      return { rawCmd: 'PFMERGE', args: [s('dest_key'), ...srcKeys] };
    }

    case 'hyperloglog.stats': return { rawCmd: 'HLLSTATS', args: [] };

    // ── Geospatial ────────────────────────────────────────────────────────────
    case 'geospatial.geoadd': {
      const members = Array.isArray(payload['members'])
        ? (payload['members'] as Array<{ lat: number; lon: number; member: string }>).flatMap(
            (m) => [String(m.lat), String(m.lon), m.member],
          )
        : [];
      return { rawCmd: 'GEOADD', args: [s('key'), ...members] };
    }

    case 'geospatial.geopos':  return { rawCmd: 'GEOPOS',  args: [s('key'), s('member')] };
    case 'geospatial.geodist': return { rawCmd: 'GEODIST', args: [s('key'), s('member1'), s('member2'), s('unit') || 'm'] };
    case 'geospatial.geohash': return { rawCmd: 'GEOHASH', args: [s('key'), s('member')] };

    case 'geospatial.georadius':
      return {
        rawCmd: 'GEORADIUS',
        args: [s('key'), n('longitude', 0), n('latitude', 0), n('radius', 0), s('unit') || 'm'],
      };

    case 'geospatial.georadiusbymember':
      return {
        rawCmd: 'GEORADIUSBYMEMBER',
        args: [s('key'), s('member'), n('radius', 0), s('unit') || 'm'],
      };

    case 'geospatial.geosearch':
      return {
        rawCmd: 'GEOSEARCH',
        args: [s('key'), 'FROMLONLAT', n('longitude', 0), n('latitude', 0), 'BYRADIUS', n('radius', 0), s('unit') || 'm'],
      };

    case 'geospatial.stats': return { rawCmd: 'GEOSTATS', args: [s('key')] };

    // ── Bitmap ────────────────────────────────────────────────────────────────
    case 'bitmap.setbit':  return { rawCmd: 'SETBIT',  args: [s('key'), n('offset', 0), n('value', 0)] };
    case 'bitmap.getbit':  return { rawCmd: 'GETBIT',  args: [s('key'), n('offset', 0)] };
    case 'bitmap.bitcount': {
      const args: unknown[] = [s('key')];
      if (payload['start'] != null && payload['end'] != null) {
        args.push(n('start', 0), n('end', -1));
      }
      return { rawCmd: 'BITCOUNT', args };
    }
    case 'bitmap.bitpos': {
      const args: unknown[] = [s('key'), n('bit', 0)];
      if (payload['start'] != null) args.push(n('start', 0));
      if (payload['end'] != null) args.push(n('end', -1));
      return { rawCmd: 'BITPOS', args };
    }
    case 'bitmap.bitop': {
      const keys = Array.isArray(payload['keys']) ? (payload['keys'] as string[]) : [];
      return { rawCmd: 'BITOP', args: [s('operation'), s('dest_key'), ...keys] };
    }

    default:
      return null;
  }
}

// ── mapResponse ────────────────────────────────────────────────────────────────

/**
 * Normalise a raw native transport response into the JSON shape that each
 * SDK manager class expects.
 *
 * The raw value originates from either:
 *  - RESP3 parser  → JS primitives / arrays
 *  - SynapRPC      → WireValue-decoded (already unwrapped by `fromWireValue`)
 *
 * @param cmd SDK command name (e.g. `"kv.set"`)
 * @param raw Raw parsed response from the wire transport
 * @returns   Normalised JS object/value matching the manager's expected shape
 */
export function mapResponse(cmd: string, raw: unknown): unknown {
  const asInt = (v: unknown, def = 0): number => {
    if (typeof v === 'number') return Math.trunc(v);
    if (typeof v === 'boolean') return v ? 1 : 0;
    return parseInt(String(v ?? def), 10);
  };

  const asFloat = (v: unknown, def = 0.0): number =>
    typeof v === 'number' ? v : parseFloat(String(v ?? def));

  const asArr = (v: unknown): unknown[] =>
    Array.isArray(v) ? v : v == null ? [] : [v];

  /** Convert interleaved `[member, score, ...]` array to `[{member, score}, ...]`. */
  const interleaved = (arr: unknown[]): Array<{ member: string; score: number }> => {
    const result: Array<{ member: string; score: number }> = [];
    for (let i = 0; i + 1 < arr.length; i += 2) {
      result.push({ member: String(arr[i]), score: asFloat(arr[i + 1]) });
    }
    return result;
  };

  switch (cmd) {
    // ── KV ────────────────────────────────────────────────────────────────────
    case 'kv.get':    return raw;
    case 'kv.set':    return {};
    case 'kv.del':    return { deleted: asInt(raw) > 0 };
    case 'kv.exists': return { exists: asInt(raw) > 0 };
    case 'kv.incr':
    case 'kv.decr':   return { value: asInt(raw) };
    case 'kv.keys':   return { keys: asArr(raw) };
    case 'kv.expire': return {};
    case 'kv.ttl':    return raw;

    // ── Hash ──────────────────────────────────────────────────────────────────
    case 'hash.set':    return { success: asInt(raw) >= 0 };
    case 'hash.get':    return { value: raw ?? null };
    case 'hash.getall': {
      const arr = asArr(raw);
      const fields: Record<string, unknown> = {};
      for (let i = 0; i + 1 < arr.length; i += 2) {
        fields[String(arr[i])] = arr[i + 1];
      }
      return { fields };
    }
    case 'hash.del':         return { deleted: asInt(raw) };
    case 'hash.exists':      return { exists: asInt(raw) > 0 };
    case 'hash.keys':        return { fields: asArr(raw) };
    case 'hash.values':      return { values: asArr(raw) };
    case 'hash.len':         return { length: asInt(raw) };
    case 'hash.mset':        return { success: raw != null };
    case 'hash.mget':        return { values: asArr(raw) };
    case 'hash.incrby':      return { value: asInt(raw) };
    case 'hash.incrbyfloat': return { value: asFloat(raw) };
    case 'hash.setnx':       return { created: asInt(raw) > 0 };

    // ── List ──────────────────────────────────────────────────────────────────
    case 'list.lpush':
    case 'list.rpush':
    case 'list.lpushx':
    case 'list.rpushx':  return { length: asInt(raw) };
    case 'list.lpop':
    case 'list.rpop':    return { values: raw == null ? [] : Array.isArray(raw) ? raw : [raw] };
    case 'list.range':   return { values: asArr(raw) };
    case 'list.len':     return { length: asInt(raw) };
    case 'list.index':   return raw;
    case 'list.set':
    case 'list.trim':    return {};
    case 'list.rem':     return { count: asInt(raw) };
    case 'list.insert':  return { length: asInt(raw) };
    case 'list.rpoplpush': return raw;
    case 'list.pos':     return raw;

    // ── Set ───────────────────────────────────────────────────────────────────
    case 'set.add':        return { added: asInt(raw) };
    case 'set.rem':        return { removed: asInt(raw) };
    case 'set.ismember':   return { is_member: asInt(raw) > 0 };
    case 'set.members':
    case 'set.pop':
    case 'set.randmember': return { members: asArr(raw) };
    case 'set.card':       return { cardinality: asInt(raw) };
    case 'set.move':       return { moved: asInt(raw) > 0 };
    case 'set.inter':
    case 'set.union':
    case 'set.diff':       return { members: asArr(raw) };
    case 'set.interstore':
    case 'set.unionstore':
    case 'set.diffstore':  return { count: asInt(raw) };

    // ── Sorted Set ────────────────────────────────────────────────────────────
    case 'sortedset.zadd':   return { added: asInt(raw) };
    case 'sortedset.zrem':   return { removed: asInt(raw) };
    case 'sortedset.zscore': return { score: raw == null ? null : asFloat(raw) };
    case 'sortedset.zcard':  return { count: asInt(raw) };
    case 'sortedset.zincrby': return { score: asFloat(raw) };
    case 'sortedset.zrank':
    case 'sortedset.zrevrank': return { rank: raw == null ? null : asInt(raw) };
    case 'sortedset.zcount':
    case 'sortedset.zremrangebyrank':
    case 'sortedset.zremrangebyscore': return { count: asInt(raw) };
    case 'sortedset.zrange':
    case 'sortedset.zrevrange':
    case 'sortedset.zrangebyscore':
    case 'sortedset.zpopmin':
    case 'sortedset.zpopmax': {
      return { members: interleaved(asArr(raw)) };
    }
    case 'sortedset.zinterstore':
    case 'sortedset.zunionstore':
    case 'sortedset.zdiffstore': return { count: asInt(raw) };

    // ── Queue ─────────────────────────────────────────────────────────────────
    case 'queue.create':
    case 'queue.delete':
    case 'queue.purge': return {};
    case 'queue.list':  return Array.isArray(raw) ? raw : raw == null ? [] : [raw];
    case 'queue.publish': {
      if (raw && typeof raw === 'object' && 'message_id' in (raw as object)) return raw;
      return { message_id: String(raw ?? '') };
    }
    case 'queue.consume': return raw ?? null;
    case 'queue.ack':
    case 'queue.nack':  return {};
    case 'queue.stats': return raw;

    // ── Stream ────────────────────────────────────────────────────────────────
    case 'stream.create':
    case 'stream.delete': return {};
    case 'stream.list':   return Array.isArray(raw) ? raw : raw == null ? [] : [raw];
    case 'stream.publish': {
      if (raw && typeof raw === 'object' && 'offset' in (raw as object)) return raw;
      return { offset: asInt(raw) };
    }
    case 'stream.consume': return Array.isArray(raw) ? { events: raw } : raw;
    case 'stream.stats':   return raw;

    // ── Pub/Sub ───────────────────────────────────────────────────────────────
    case 'pubsub.publish': {
      if (raw && typeof raw === 'object' && 'subscribers_matched' in (raw as object)) return raw;
      return { message_id: '', subscribers_matched: asInt(raw) };
    }
    case 'pubsub.subscribe':   return raw;
    case 'pubsub.unsubscribe': return {};
    case 'pubsub.topics':
    case 'pubsub.list': return Array.isArray(raw) ? { topics: raw } : (raw ?? { topics: [] });

    // ── Transactions ──────────────────────────────────────────────────────────
    case 'transaction.multi':
    case 'transaction.discard':
    case 'transaction.watch':
    case 'transaction.unwatch': return raw ?? { success: true };
    case 'transaction.exec':    return raw;

    // ── Scripts ───────────────────────────────────────────────────────────────
    case 'script.eval':
    case 'script.evalsha': return raw;
    case 'script.load': {
      if (raw && typeof raw === 'object' && 'sha1' in (raw as object)) return raw;
      return { sha1: String(raw ?? '') };
    }
    case 'script.exists': {
      if (raw && typeof raw === 'object' && 'exists' in (raw as object)) return raw;
      return { exists: Array.isArray(raw) ? raw.map(Boolean) : [] };
    }
    case 'script.flush': return raw ?? { cleared: 0 };
    case 'script.kill':  return raw ?? { terminated: false };

    // ── HyperLogLog ───────────────────────────────────────────────────────────
    case 'hyperloglog.pfadd':   return { changed: asInt(raw) > 0 };
    case 'hyperloglog.pfcount': return { count: asInt(raw) };
    case 'hyperloglog.pfmerge': return {};
    case 'hyperloglog.stats':   return raw;

    // ── Geospatial ────────────────────────────────────────────────────────────
    case 'geospatial.geoadd':  return { added: asInt(raw) };
    case 'geospatial.geopos':  return raw;
    case 'geospatial.geodist': return { distance: raw == null ? null : asFloat(raw) };
    case 'geospatial.geohash': return { hash: raw };
    case 'geospatial.georadius':
    case 'geospatial.georadiusbymember':
    case 'geospatial.geosearch': return { members: asArr(raw) };
    case 'geospatial.stats':   return raw;

    // ── Bitmap ────────────────────────────────────────────────────────────────
    case 'bitmap.setbit':   return { original: asInt(raw) };
    case 'bitmap.getbit':   return { bit: asInt(raw) };
    case 'bitmap.bitcount': return { count: asInt(raw) };
    case 'bitmap.bitpos':   return { position: asInt(raw, -1) };
    case 'bitmap.bitop':    return { length: asInt(raw) };

    default:
      return raw;
  }
}

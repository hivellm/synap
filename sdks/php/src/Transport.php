<?php

declare(strict_types=1);

namespace Synap\SDK;

use MessagePack\MessagePack;
use Synap\SDK\Exception\SynapException;

/**
 * Transport mode constants.
 */
final class TransportMode
{
    public const SYNAP_RPC = 'synaprpc';
    public const RESP3 = 'resp3';
    public const HTTP = 'http';
}

// ── Wire value helpers ────────────────────────────────────────────────────────

/**
 * Wrap a PHP value in the externally-tagged WireValue envelope (rmp_serde format).
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
    // Fallback: stringify
    return ['Str' => (string) $v];
}

/**
 * Unwrap a WireValue envelope back to a plain PHP value.
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

// ── Command mapping ───────────────────────────────────────────────────────────

/**
 * Map a dotted SDK command + payload to a native wire command + args.
 *
 * Returns null for unmapped commands (fall back to HTTP).
 *
 * @param string $cmd
 * @param array<string, mixed> $payload
 * @return array{0: string, 1: list<mixed>}|null
 */
function mapCommand(string $cmd, array $payload): ?array
{
    $key = (string) ($payload['key'] ?? '');
    $value = $payload['value'] ?? null;
    $field = (string) ($payload['field'] ?? '');
    $fields = (array) ($payload['fields'] ?? []);
    $ttl = $payload['ttl'] ?? null;

    switch ($cmd) {
        // ── KV ──────────────────────────────────────────────────────────────
        case 'kv.get':
            return ['GET', [$key]];
        case 'kv.set':
            if ($ttl !== null) {
                return ['SET', [$key, $value, 'EX', $ttl]];
            }
            return ['SET', [$key, $value]];
        case 'kv.del':
            return ['DEL', [$key]];
        case 'kv.exists':
            return ['EXISTS', [$key]];
        case 'kv.expire':
            return ['EXPIRE', [$key, $payload['seconds'] ?? 0]];
        case 'kv.ttl':
            return ['TTL', [$key]];
        case 'kv.persist':
            return ['PERSIST', [$key]];
        case 'kv.incr':
            return ['INCR', [$key]];
        case 'kv.incrby':
            return ['INCRBY', [$key, $payload['amount'] ?? 1]];
        case 'kv.decr':
            return ['DECR', [$key]];
        case 'kv.decrby':
            return ['DECRBY', [$key, $payload['amount'] ?? 1]];
        case 'kv.append':
            return ['APPEND', [$key, $value]];
        case 'kv.strlen':
            return ['STRLEN', [$key]];
        case 'kv.getset':
            return ['GETSET', [$key, $value]];
        case 'kv.setnx':
            return ['SETNX', [$key, $value]];
        case 'kv.scan':
            return ['SCAN', [$payload['cursor'] ?? 0, 'MATCH', $payload['pattern'] ?? '*', 'COUNT', $payload['count'] ?? 100]];
        case 'kv.keys':
            return ['KEYS', [$payload['pattern'] ?? '*']];
        case 'kv.type':
            return ['TYPE', [$key]];
        case 'kv.rename':
            return ['RENAME', [$key, $payload['new_key'] ?? '']];
        case 'kv.copy':
            return ['COPY', [$key, $payload['destination'] ?? '']];
        // ── Hash ─────────────────────────────────────────────────────────────
        case 'hash.get':
            return ['HGET', [$key, $field]];
        case 'hash.set':
            return ['HSET', [$key, $field, $value]];
        case 'hash.del':
            return ['HDEL', [$key, $field]];
        case 'hash.exists':
            return ['HEXISTS', [$key, $field]];
        case 'hash.getall':
            return ['HGETALL', [$key]];
        case 'hash.keys':
            return ['HKEYS', [$key]];
        case 'hash.values':
            return ['HVALS', [$key]];
        case 'hash.len':
            return ['HLEN', [$key]];
        case 'hash.mget':
            return ['HMGET', array_merge([$key], (array) ($payload['fields'] ?? []))];
        case 'hash.mset':
            $args = [$key];
            foreach ($fields as $k => $v) {
                $args[] = $k;
                $args[] = $v;
            }
            return ['HMSET', $args];
        case 'hash.incrby':
            return ['HINCRBY', [$key, $field, $payload['amount'] ?? 1]];
        case 'hash.incrbyfloat':
            return ['HINCRBYFLOAT', [$key, $field, $payload['amount'] ?? 1.0]];
        case 'hash.setnx':
            return ['HSETNX', [$key, $field, $value]];
        // ── List ─────────────────────────────────────────────────────────────
        case 'list.lpush':
            return ['LPUSH', [$key, $value]];
        case 'list.rpush':
            return ['RPUSH', [$key, $value]];
        case 'list.lpop':
            return ['LPOP', [$key]];
        case 'list.rpop':
            return ['RPOP', [$key]];
        case 'list.lrange':
            return ['LRANGE', [$key, $payload['start'] ?? 0, $payload['stop'] ?? -1]];
        case 'list.llen':
            return ['LLEN', [$key]];
        case 'list.lindex':
            return ['LINDEX', [$key, $payload['index'] ?? 0]];
        case 'list.lset':
            return ['LSET', [$key, $payload['index'] ?? 0, $value]];
        case 'list.lrem':
            return ['LREM', [$key, $payload['count'] ?? 0, $value]];
        case 'list.ltrim':
            return ['LTRIM', [$key, $payload['start'] ?? 0, $payload['stop'] ?? -1]];
        case 'list.lpos':
            return ['LPOS', [$key, $value]];
        // ── Set ──────────────────────────────────────────────────────────────
        case 'set.add':
            return ['SADD', [$key, $value]];
        case 'set.remove':
            return ['SREM', [$key, $value]];
        case 'set.members':
            return ['SMEMBERS', [$key]];
        case 'set.ismember':
            return ['SISMEMBER', [$key, $value]];
        case 'set.card':
            return ['SCARD', [$key]];
        case 'set.pop':
            return ['SPOP', [$key]];
        case 'set.randmember':
            return ['SRANDMEMBER', [$key, $payload['count'] ?? 1]];
        case 'set.union':
            return ['SUNION', array_merge([$key], (array) ($payload['keys'] ?? []))];
        case 'set.inter':
            return ['SINTER', array_merge([$key], (array) ($payload['keys'] ?? []))];
        case 'set.diff':
            return ['SDIFF', array_merge([$key], (array) ($payload['keys'] ?? []))];
        case 'set.move':
            return ['SMOVE', [$key, $payload['destination'] ?? '', $value]];
        // ── Sorted Set ───────────────────────────────────────────────────────
        case 'sorted_set.add':
            return ['ZADD', [$key, $payload['score'] ?? 0.0, $value]];
        case 'sorted_set.score':
            return ['ZSCORE', [$key, $value]];
        case 'sorted_set.rank':
            return ['ZRANK', [$key, $value]];
        case 'sorted_set.revrank':
            return ['ZREVRANK', [$key, $value]];
        case 'sorted_set.range':
            return ['ZRANGE', [$key, $payload['start'] ?? 0, $payload['stop'] ?? -1, 'WITHSCORES']];
        case 'sorted_set.revrange':
            return ['ZREVRANGE', [$key, $payload['start'] ?? 0, $payload['stop'] ?? -1, 'WITHSCORES']];
        case 'sorted_set.card':
            return ['ZCARD', [$key]];
        case 'sorted_set.count':
            return ['ZCOUNT', [$key, $payload['min'] ?? '-inf', $payload['max'] ?? '+inf']];
        case 'sorted_set.rem':
            return ['ZREM', [$key, $value]];
        case 'sorted_set.incrby':
            return ['ZINCRBY', [$key, $payload['increment'] ?? 1.0, $value]];

        // ── Queue ─────────────────────────────────────────────────────────────
        case 'queue.create':
            return ['QCREATE', [
                (string) ($payload['name'] ?? ''),
                (int)    ($payload['max_depth'] ?? 0),
                (int)    ($payload['ack_deadline_secs'] ?? 0),
            ]];
        case 'queue.delete':
            return ['QDELETE', [(string) ($payload['queue'] ?? '')]];
        case 'queue.publish': {
            $args = [
                (string) ($payload['queue'] ?? ''),
                json_encode($payload['payload'] ?? null),
                (int)    ($payload['priority'] ?? 0),
                (int)    ($payload['max_retries'] ?? 3),
            ];
            return ['QPUBLISH', $args];
        }
        case 'queue.consume':
            return ['QCONSUME', [
                (string) ($payload['queue'] ?? ''),
                (string) ($payload['consumer_id'] ?? ''),
            ]];
        case 'queue.ack':
            return ['QACK', [
                (string) ($payload['queue'] ?? ''),
                (string) ($payload['message_id'] ?? ''),
            ]];
        case 'queue.nack':
            return ['QNACK', [
                (string) ($payload['queue'] ?? ''),
                (string) ($payload['message_id'] ?? ''),
                (int)    ($payload['delay_secs'] ?? 0),
            ]];
        case 'queue.stats':
            return ['QSTATS', [(string) ($payload['queue'] ?? '')]];
        case 'queue.purge':
            return ['QPURGE', [(string) ($payload['queue'] ?? '')]];
        case 'queue.list':
            return ['QLIST', []];

        // ── Stream ────────────────────────────────────────────────────────────
        case 'stream.create':
            return ['SCREATE', [(string) ($payload['room'] ?? '')]];
        case 'stream.delete':
            return ['SDELETE', [(string) ($payload['room'] ?? '')]];
        case 'stream.publish':
            return ['SPUBLISH', [
                (string) ($payload['room'] ?? ''),
                (string) ($payload['event'] ?? ''),
                json_encode($payload['data'] ?? null),
            ]];
        case 'stream.consume':
            return ['SREAD', [
                (string) ($payload['room'] ?? ''),
                (string) ($payload['subscriber_id'] ?? 'sdk-reader'),
                (string) ($payload['from_offset'] ?? 0),
            ]];
        case 'stream.list':
            return ['SLIST', []];

        // ── Pub/Sub ───────────────────────────────────────────────────────────
        case 'pubsub.publish':
            return ['PPUBLISH', [
                (string) ($payload['topic'] ?? ''),
                json_encode($payload['payload'] ?? null),
            ]];
        case 'pubsub.subscribe':
            return ['PSUBSCRIBE', array_merge(
                [(string) ($payload['subscriber_id'] ?? '')],
                (array) ($payload['topics'] ?? [])
            )];
        case 'pubsub.unsubscribe':
            return ['PUNSUBSCRIBE', array_merge(
                [(string) ($payload['subscriber_id'] ?? '')],
                (array) ($payload['topics'] ?? [])
            )];
        case 'pubsub.topics':
            return ['PTOPICS', []];
        case 'pubsub.stats':
            return ['PSTATS', []];

        // ── Transaction ───────────────────────────────────────────────────────
        case 'transaction.multi':
            return ['MULTI', [(string) ($payload['client_id'] ?? '')]];
        case 'transaction.exec':
            return ['EXEC', [(string) ($payload['client_id'] ?? '')]];
        case 'transaction.discard':
            return ['DISCARD', [(string) ($payload['client_id'] ?? '')]];
        case 'transaction.watch':
            return ['WATCH', array_merge(
                [(string) ($payload['client_id'] ?? '')],
                (array) ($payload['keys'] ?? [])
            )];
        case 'transaction.unwatch':
            return ['UNWATCH', [(string) ($payload['client_id'] ?? '')]];

        // ── Scripting ─────────────────────────────────────────────────────────
        case 'script.eval':
            return ['EVAL', [
                (string) ($payload['script'] ?? ''),
                count((array) ($payload['keys'] ?? [])),
                ...array_merge(
                    (array) ($payload['keys'] ?? []),
                    (array) ($payload['args'] ?? [])
                ),
            ]];
        case 'script.evalsha':
            return ['EVALSHA', [
                (string) ($payload['sha'] ?? ''),
                count((array) ($payload['keys'] ?? [])),
                ...array_merge(
                    (array) ($payload['keys'] ?? []),
                    (array) ($payload['args'] ?? [])
                ),
            ]];

        // ── HyperLogLog ───────────────────────────────────────────────────────
        case 'hyperloglog.pfadd':
            return ['PFADD', array_merge([$key], (array) ($payload['elements'] ?? []))];
        case 'hyperloglog.pfcount':
            return ['PFCOUNT', array_merge([$key], (array) ($payload['keys'] ?? []))];
        case 'hyperloglog.pfmerge':
            return ['PFMERGE', array_merge(
                [(string) ($payload['destination'] ?? '')],
                [$key],
                (array) ($payload['sources'] ?? [])
            )];

        // ── Geospatial ────────────────────────────────────────────────────────
        case 'geo.add':
            return ['GEOADD', [
                $key,
                (float) ($payload['longitude'] ?? 0.0),
                (float) ($payload['latitude'] ?? 0.0),
                (string) ($payload['member'] ?? ''),
            ]];
        case 'geo.dist':
            return ['GEODIST', [
                $key,
                (string) ($payload['member1'] ?? ''),
                (string) ($payload['member2'] ?? ''),
                (string) ($payload['unit'] ?? 'm'),
            ]];
        case 'geo.pos':
            return ['GEOPOS', [$key, (string) ($payload['member'] ?? '')]];
        case 'geo.radius':
            return ['GEORADIUS', [
                $key,
                (float) ($payload['longitude'] ?? 0.0),
                (float) ($payload['latitude'] ?? 0.0),
                (float) ($payload['radius'] ?? 0.0),
                (string) ($payload['unit'] ?? 'm'),
                'WITHCOORD', 'WITHDIST', 'ASC',
                'COUNT', (int) ($payload['count'] ?? 100),
            ]];
        case 'geo.search':
            return ['GEOSEARCH', [
                $key,
                'FROMMEMBER', (string) ($payload['member'] ?? ''),
                'BYRADIUS', (float) ($payload['radius'] ?? 0.0), (string) ($payload['unit'] ?? 'm'),
                'ASC',
                'COUNT', (int) ($payload['count'] ?? 100),
            ]];
        case 'geo.hash':
            return ['GEOHASH', [$key, (string) ($payload['member'] ?? '')]];

        // ── KV stats ─────────────────────────────────────────────────────────
        case 'kv.stats':
            return ['INFO', ['keyspace']];

        default:
            return null;
    }
}

/**
 * Map a raw wire response to the array shape that SDK modules expect.
 *
 * @param string $cmd
 * @param mixed $raw
 * @return array<string, mixed>
 */
function mapResponse(string $cmd, mixed $raw): array
{
    switch ($cmd) {
        case 'kv.get':
            return ['value' => $raw];
        case 'kv.set':
            return ['success' => $raw === 'OK' || $raw === true];
        case 'kv.del':
            return ['deleted' => (bool) $raw];
        case 'kv.exists':
            return ['exists' => (bool) $raw];
        case 'kv.expire':
        case 'kv.persist':
            return ['success' => (bool) $raw];
        case 'kv.ttl':
            return ['ttl' => $raw];
        case 'kv.incr':
        case 'kv.incrby':
        case 'kv.decr':
        case 'kv.decrby':
            return ['value' => $raw];
        case 'kv.append':
        case 'kv.strlen':
            return ['length' => $raw];
        case 'kv.setnx':
            return ['success' => (bool) $raw];
        case 'kv.getset':
            return ['old_value' => $raw];
        case 'kv.scan':
            if (is_array($raw) && count($raw) >= 2) {
                return ['cursor' => $raw[0], 'keys' => (array) $raw[1]];
            }
            return ['cursor' => 0, 'keys' => []];
        case 'kv.keys':
            return ['keys' => is_array($raw) ? $raw : []];
        case 'kv.type':
            return ['type' => $raw];
        case 'kv.rename':
        case 'kv.copy':
            return ['success' => $raw === 'OK' || $raw === true];
        case 'hash.get':
            return ['value' => $raw];
        case 'hash.set':
        case 'hash.setnx':
            return ['success' => (bool) $raw];
        case 'hash.del':
            return ['deleted' => (bool) $raw];
        case 'hash.exists':
            return ['exists' => (bool) $raw];
        case 'hash.getall':
            if (is_array($raw) && array_is_list($raw)) {
                $fields = [];
                for ($i = 0; $i < count($raw) - 1; $i += 2) {
                    $fields[(string) $raw[$i]] = $raw[$i + 1];
                }
                return ['fields' => $fields];
            }
            return ['fields' => is_array($raw) ? $raw : []];
        case 'hash.keys':
            return ['keys' => is_array($raw) ? $raw : []];
        case 'hash.values':
            return ['values' => is_array($raw) ? $raw : []];
        case 'hash.len':
            return ['length' => $raw];
        case 'hash.mget':
            return ['values' => is_array($raw) ? $raw : []];
        case 'hash.mset':
            return ['success' => $raw === 'OK' || $raw === true];
        case 'hash.incrby':
        case 'hash.incrbyfloat':
            return ['value' => $raw];
        case 'list.lpush':
        case 'list.rpush':
            return ['length' => $raw];
        case 'list.lpop':
        case 'list.rpop':
            return ['value' => $raw];
        case 'list.lrange':
            return ['values' => is_array($raw) ? $raw : []];
        case 'list.llen':
            return ['length' => $raw];
        case 'list.lindex':
            return ['value' => $raw];
        case 'list.lset':
        case 'list.ltrim':
            return ['success' => $raw === 'OK' || $raw === true];
        case 'list.lrem':
            return ['removed' => $raw];
        case 'list.lpos':
            return ['index' => $raw];
        case 'set.add':
            return ['added' => $raw];
        case 'set.remove':
            return ['removed' => $raw];
        case 'set.members':
            return ['members' => is_array($raw) ? $raw : []];
        case 'set.ismember':
            return ['is_member' => (bool) $raw];
        case 'set.card':
            return ['cardinality' => $raw];
        case 'set.pop':
            return ['value' => $raw];
        case 'set.randmember':
            return ['members' => is_array($raw) ? $raw : [$raw]];
        case 'set.union':
        case 'set.inter':
        case 'set.diff':
            return ['members' => is_array($raw) ? $raw : []];
        case 'set.move':
            return ['success' => (bool) $raw];
        case 'sorted_set.add':
            return ['added' => $raw];
        case 'sorted_set.rem':
            return ['removed' => $raw];
        case 'sorted_set.score':
            return ['score' => $raw !== null ? (float) $raw : null];
        case 'sorted_set.rank':
        case 'sorted_set.revrank':
            return ['rank' => $raw];
        case 'sorted_set.range':
        case 'sorted_set.revrange':
        case 'sorted_set.rangebyscore':
            if (is_array($raw) && array_is_list($raw)) {
                $members = [];
                for ($i = 0; $i < count($raw) - 1; $i += 2) {
                    $members[] = ['member' => (string) $raw[$i], 'score' => (float) $raw[$i + 1]];
                }
                return ['members' => $members];
            }
            return ['members' => []];
        case 'sorted_set.card':
            return ['cardinality' => $raw];
        case 'sorted_set.count':
            return ['count' => $raw];
        case 'sorted_set.incrby':
            return ['score' => (float) $raw];

        // ── Queue ─────────────────────────────────────────────────────────────
        case 'queue.create':
            return ['success' => $raw === 'OK' || $raw === true];
        case 'queue.delete':
        case 'queue.purge':
            return ['success' => (bool) $raw];
        case 'queue.publish':
            return ['message_id' => is_string($raw) ? $raw : (string) ($raw ?? '')];
        case 'queue.consume':
            if ($raw === null || $raw === 'Null') {
                return [];
            }
            if (is_array($raw)) {
                return ['message' => $raw];
            }
            return [];
        case 'queue.ack':
        case 'queue.nack':
            return ['success' => (bool) $raw];
        case 'queue.stats':
            return is_array($raw) ? $raw : ['result' => $raw];
        case 'queue.list':
            return ['queues' => is_array($raw) ? $raw : []];

        // ── Stream ────────────────────────────────────────────────────────────
        case 'stream.create':
            return ['success' => $raw === 'OK' || $raw === true];
        case 'stream.delete':
            return ['success' => (bool) $raw];
        case 'stream.publish':
            return ['offset' => is_int($raw) ? $raw : (int) ($raw ?? 0)];
        case 'stream.consume':
            return ['events' => is_array($raw) ? $raw : []];
        case 'stream.list':
            return ['rooms' => is_array($raw) ? $raw : []];

        // ── Pub/Sub ───────────────────────────────────────────────────────────
        case 'pubsub.publish':
            return ['subscribers_matched' => is_int($raw) ? $raw : (int) ($raw ?? 0)];
        case 'pubsub.subscribe':
            return ['success' => (bool) $raw];
        case 'pubsub.unsubscribe':
            return ['success' => (bool) $raw];
        case 'pubsub.topics':
            return ['topics' => is_array($raw) ? $raw : []];
        case 'pubsub.stats':
            return is_array($raw) ? $raw : ['result' => $raw];

        // ── Transaction ───────────────────────────────────────────────────────
        case 'transaction.multi':
        case 'transaction.discard':
        case 'transaction.watch':
        case 'transaction.unwatch':
            return ['success' => $raw === 'OK' || $raw === true];
        case 'transaction.exec':
            return ['success' => true, 'results' => is_array($raw) ? $raw : []];

        // ── Scripting ─────────────────────────────────────────────────────────
        case 'script.eval':
        case 'script.evalsha':
            return ['result' => $raw];

        // ── HyperLogLog ───────────────────────────────────────────────────────
        case 'hyperloglog.pfadd':
            return ['changed' => (bool) $raw];
        case 'hyperloglog.pfcount':
            return ['count' => is_int($raw) ? $raw : (int) ($raw ?? 0)];
        case 'hyperloglog.pfmerge':
            return ['success' => $raw === 'OK' || $raw === true];

        // ── Geospatial ────────────────────────────────────────────────────────
        case 'geo.add':
            return ['added' => is_int($raw) ? $raw : (int) ($raw ?? 0)];
        case 'geo.dist':
            return ['distance' => $raw !== null ? (float) $raw : null];
        case 'geo.pos':
            return ['position' => is_array($raw) ? $raw : null];
        case 'geo.radius':
        case 'geo.search':
            return ['members' => is_array($raw) ? $raw : []];
        case 'geo.hash':
            return ['hash' => is_string($raw) ? $raw : null];

        // ── KV stats ─────────────────────────────────────────────────────────
        case 'kv.stats':
            return is_array($raw) ? $raw : ['result' => $raw];

        default:
            if (is_array($raw)) {
                return $raw;
            }
            return ['result' => $raw];
    }
}

// ── SynapRPC transport ─────────────────────────────────────────────────────────

/**
 * Blocking TCP transport using the SynapRPC MessagePack protocol.
 */
class SynapRpcTransport
{
    /** @var resource|null */
    private mixed $socket = null;
    private int $nextId = 1;

    public function __construct(
        private readonly string $host,
        private readonly int $port,
        private readonly int $timeoutSecs,
    ) {}

    /** @return resource */
    private function ensureConnected(): mixed
    {
        if ($this->socket !== null) {
            return $this->socket;
        }

        $errno = 0;
        $errstr = '';
        $sock = @fsockopen($this->host, $this->port, $errno, $errstr, $this->timeoutSecs);
        if ($sock === false) {
            throw SynapException::networkError("SynapRPC connect failed ({$errno}): {$errstr}");
        }
        stream_set_timeout($sock, $this->timeoutSecs);
        $this->socket = $sock;

        return $sock;
    }

    /**
     * Execute a command and return the decoded response value.
     *
     * @param string $cmd
     * @param list<mixed> $args
     * @return mixed
     */
    public function execute(string $cmd, array $args): mixed
    {
        $sock = $this->ensureConnected();
        $id = $this->nextId++;
        $wireArgs = array_map(__NAMESPACE__ . '\\toWireValue', $args);

        // Encode request as msgpack array: [id, CMD, args]
        $body = MessagePack::pack([$id, strtoupper($cmd), $wireArgs]);
        $frame = pack('V', strlen($body)) . $body; // little-endian u32 length prefix

        if (fwrite($sock, $frame) === false) {
            $this->socket = null;
            throw SynapException::networkError('SynapRPC write failed');
        }

        // Read response frame: 4-byte LE u32 length
        $lenBytes = $this->readExact($sock, 4);
        $frameLen = unpack('V', $lenBytes)[1];
        $responseBody = $this->readExact($sock, $frameLen);

        $decoded = MessagePack::unpack($responseBody);
        // Response: [id, {Ok: wire_value} | {Err: string}]
        [, $resultEnv] = $decoded;

        if (isset($resultEnv['Ok'])) {
            return fromWireValue($resultEnv['Ok']);
        }
        $errMsg = is_string($resultEnv['Err'] ?? null) ? $resultEnv['Err'] : 'unknown server error';
        throw SynapException::serverError($errMsg);
    }

    /**
     * @param resource $sock
     */
    private function readExact(mixed $sock, int $n): string
    {
        $buf = '';
        $remaining = $n;
        while ($remaining > 0) {
            $chunk = fread($sock, $remaining);
            if ($chunk === false || $chunk === '') {
                $this->socket = null;
                throw SynapException::networkError('SynapRPC connection closed unexpectedly');
            }
            $buf .= $chunk;
            $remaining -= strlen($chunk);
        }
        return $buf;
    }

    /**
     * Open a dedicated server-push connection, send a SUBSCRIBE frame, and
     * block calling the callback for each push message received.
     *
     * Push frames use id == 0xFFFFFFFF (U32_MAX) as a sentinel.
     * The loop runs until $shouldStop returns true or the connection closes.
     *
     * @param list<string>      $topics     Topic patterns to subscribe to
     * @param callable          $onMessage  Invoked with each push message array
     * @param callable|null     $shouldStop Optional predicate; loop exits when it returns true
     */
    public function subscribePush(array $topics, callable $onMessage, ?callable $shouldStop = null): void
    {
        $errno  = 0;
        $errstr = '';
        /** @var resource|false $pushSock */
        $pushSock = @fsockopen($this->host, $this->port, $errno, $errstr, $this->timeoutSecs);
        if ($pushSock === false) {
            throw SynapException::networkError("SynapRPC push connect failed ({$errno}): {$errstr}");
        }
        stream_set_timeout($pushSock, $this->timeoutSecs);

        // Send SUBSCRIBE frame: [id=0xFFFFFFFF, "SUBSCRIBE", [topic, ...]]
        $PUSH_ID = 0xFFFF_FFFF;
        $wireTopics = array_map(__NAMESPACE__ . '\\toWireValue', $topics);
        $body  = MessagePack::pack([$PUSH_ID, 'SUBSCRIBE', $wireTopics]);
        $frame = pack('V', strlen($body)) . $body;

        if (fwrite($pushSock, $frame) === false) {
            fclose($pushSock);
            throw SynapException::networkError('SynapRPC push SUBSCRIBE write failed');
        }

        // Read the initial SUBSCRIBE acknowledgement (id will be PUSH_ID)
        $lenBytes = $this->readExactFrom($pushSock, 4);
        $frameLen = unpack('V', $lenBytes)[1];
        $respBody = $this->readExactFrom($pushSock, $frameLen);
        // Ignore the initial ack value — any server error is encoded as {Err:...}
        $ack = MessagePack::unpack($respBody);
        if (isset($ack[1]['Err'])) {
            fclose($pushSock);
            throw SynapException::serverError((string) $ack[1]['Err']);
        }

        // Read push frames in a blocking loop
        try {
            while (true) {
                if ($shouldStop !== null && ($shouldStop)()) {
                    break;
                }

                $lenBytes = @fread($pushSock, 4);
                if ($lenBytes === false || strlen($lenBytes) < 4) {
                    break; // Connection closed
                }

                $frameLen  = unpack('V', $lenBytes)[1];
                $pushBody  = $this->readExactFrom($pushSock, $frameLen);
                $decoded   = MessagePack::unpack($pushBody);
                [$frameId, $resultEnv] = $decoded;

                if ((int) $frameId !== $PUSH_ID) {
                    continue; // Skip non-push frames
                }

                $value = fromWireValue($resultEnv['Ok'] ?? null);
                if (is_array($value)) {
                    ($onMessage)($value);
                }
            }
        } finally {
            fclose($pushSock);
        }
    }

    /**
     * Read exactly $n bytes from a socket resource.
     *
     * @param resource $sock
     */
    private function readExactFrom(mixed $sock, int $n): string
    {
        $buf = '';
        $remaining = $n;
        while ($remaining > 0) {
            $chunk = fread($sock, $remaining);
            if ($chunk === false || $chunk === '') {
                throw SynapException::networkError('SynapRPC connection closed unexpectedly');
            }
            $buf .= $chunk;
            $remaining -= strlen($chunk);
        }
        return $buf;
    }

    public function close(): void
    {
        if ($this->socket !== null) {
            fclose($this->socket);
            $this->socket = null;
        }
    }
}

// ── RESP3 transport ───────────────────────────────────────────────────────────

/**
 * Blocking TCP transport using the RESP3 (Redis-compatible) text protocol.
 */
class Resp3Transport
{
    /** @var resource|null */
    private mixed $socket = null;

    public function __construct(
        private readonly string $host,
        private readonly int $port,
        private readonly int $timeoutSecs,
    ) {}

    /** @return resource */
    private function ensureConnected(): mixed
    {
        if ($this->socket !== null) {
            return $this->socket;
        }

        $errno = 0;
        $errstr = '';
        $sock = @fsockopen($this->host, $this->port, $errno, $errstr, $this->timeoutSecs);
        if ($sock === false) {
            throw SynapException::networkError("RESP3 connect failed ({$errno}): {$errstr}");
        }
        stream_set_timeout($sock, $this->timeoutSecs);
        $this->socket = $sock;

        // Send HELLO 3 to upgrade to RESP3
        $hello = "*2\r\n\$5\r\nHELLO\r\n\$1\r\n3\r\n";
        fwrite($sock, $hello);
        // Drain the HELLO response
        $this->readValue($sock);

        return $sock;
    }

    /**
     * Execute a command and return the parsed response.
     *
     * @param string $cmd
     * @param list<mixed> $args
     * @return mixed
     */
    public function execute(string $cmd, array $args): mixed
    {
        $sock = $this->ensureConnected();
        $parts = array_merge([strtoupper($cmd)], $args);
        $frame = '*' . count($parts) . "\r\n";
        foreach ($parts as $part) {
            $encoded = (string) $part;
            $frame .= '$' . strlen($encoded) . "\r\n" . $encoded . "\r\n";
        }
        fwrite($sock, $frame);
        return $this->readValue($sock);
    }

    /**
     * @param resource $sock
     * @return mixed
     */
    private function readValue(mixed $sock): mixed
    {
        $line = fgets($sock);
        if ($line === false) {
            $this->socket = null;
            throw SynapException::networkError('RESP3 connection closed');
        }
        $line = rtrim($line, "\r\n");
        $prefix = $line[0];
        $rest = substr($line, 1);

        switch ($prefix) {
            case '+':
                return $rest;
            case '-':
                throw SynapException::serverError($rest);
            case ':':
                return (int) $rest;
            case ',':
                return (float) $rest;
            case '#':
                return strtolower($rest) === 't';
            case '_':
                return null;
            case '$':
                $len = (int) $rest;
                if ($len === -1) {
                    return null;
                }
                $data = $this->readExact($sock, $len + 2);
                return substr($data, 0, $len);
            case '*':
                $count = (int) $rest;
                if ($count === -1) {
                    return null;
                }
                $arr = [];
                for ($i = 0; $i < $count; $i++) {
                    $arr[] = $this->readValue($sock);
                }
                return $arr;
            case '%':
                // RESP3 map
                $count = (int) $rest;
                $map = [];
                for ($i = 0; $i < $count; $i++) {
                    $k = (string) $this->readValue($sock);
                    $map[$k] = $this->readValue($sock);
                }
                return $map;
            case '~':
                // RESP3 set type
                $count = (int) $rest;
                $set = [];
                for ($i = 0; $i < $count; $i++) {
                    $set[] = $this->readValue($sock);
                }
                return $set;
            default:
                return $rest;
        }
    }

    /**
     * @param resource $sock
     */
    private function readExact(mixed $sock, int $n): string
    {
        $buf = '';
        $remaining = $n;
        while ($remaining > 0) {
            $chunk = fread($sock, $remaining);
            if ($chunk === false || $chunk === '') {
                $this->socket = null;
                throw SynapException::networkError('RESP3 connection closed unexpectedly');
            }
            $buf .= $chunk;
            $remaining -= strlen($chunk);
        }
        return $buf;
    }

    public function close(): void
    {
        if ($this->socket !== null) {
            fclose($this->socket);
            $this->socket = null;
        }
    }
}

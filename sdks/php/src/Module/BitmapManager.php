<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * Bitmap operations (Redis-compatible)
 */
class BitmapManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Set bit at offset to value (SETBIT)
     *
     * @param int $offset Bit offset (0-based)
     * @param int $value Bit value (0 or 1)
     * @return int Previous bit value (0 or 1)
     */
    public function setBit(string $key, int $offset, int $value): int
    {
        if ($value !== 0 && $value !== 1) {
            throw new \InvalidArgumentException('Bitmap value must be 0 or 1');
        }

        $response = $this->client->execute('bitmap.setbit', $key, [
            'offset' => $offset,
            'value' => $value,
        ]);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var int|mixed $oldValue */
        $oldValue = $payload['old_value'] ?? 0;
        return (int) $oldValue;
    }

    /**
     * Get bit at offset (GETBIT)
     *
     * @return int Bit value (0 or 1)
     */
    public function getBit(string $key, int $offset): int
    {
        $response = $this->client->execute('bitmap.getbit', $key, ['offset' => $offset]);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var int|mixed $value */
        $value = $payload['value'] ?? 0;
        return (int) $value;
    }

    /**
     * Count set bits in bitmap (BITCOUNT)
     *
     * @param int|null $start Optional start offset (inclusive)
     * @param int|null $end Optional end offset (inclusive)
     * @return int Number of set bits
     */
    public function bitCount(string $key, ?int $start = null, ?int $end = null): int
    {
        $data = [];
        if ($start !== null) {
            $data['start'] = $start;
        }
        if ($end !== null) {
            $data['end'] = $end;
        }

        $response = $this->client->execute('bitmap.bitcount', $key, $data);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var int|mixed $count */
        $count = $payload['count'] ?? 0;
        return (int) $count;
    }

    /**
     * Find first bit set to value (BITPOS)
     *
     * @param int $value Bit value to search for (0 or 1)
     * @param int|null $start Optional start offset (inclusive)
     * @param int|null $end Optional end offset (inclusive)
     * @return int|null Position of first matching bit, or null if not found
     */
    public function bitPos(string $key, int $value, ?int $start = null, ?int $end = null): ?int
    {
        if ($value !== 0 && $value !== 1) {
            throw new \InvalidArgumentException('Bitmap value must be 0 or 1');
        }

        $data = ['value' => $value];
        if ($start !== null) {
            $data['start'] = $start;
        }
        if ($end !== null) {
            $data['end'] = $end;
        }

        $response = $this->client->execute('bitmap.bitpos', $key, $data);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        $position = $payload['position'] ?? null;
        if ($position === null) {
            return null;
        }
        /** @var int|mixed $position */
        return (int) $position;
    }

    /**
     * Perform bitwise operation on multiple bitmaps (BITOP)
     *
     * @param 'AND'|'OR'|'XOR'|'NOT' $operation Bitwise operation
     * @param string $destination Destination key for result
     * @param list<string> $sourceKeys Source bitmap keys (NOT requires exactly 1 source)
     * @return int Length of resulting bitmap in bits
     */
    public function bitOp(string $operation, string $destination, array $sourceKeys): int
    {
        if ($operation === 'NOT' && count($sourceKeys) !== 1) {
            throw new \InvalidArgumentException('NOT operation requires exactly one source key');
        }

        if (empty($sourceKeys)) {
            throw new \InvalidArgumentException('BITOP requires at least one source key');
        }

        $response = $this->client->execute('bitmap.bitop', $destination, [
            'operation' => $operation,
            'source_keys' => $sourceKeys,
        ]);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var int|mixed $length */
        $length = $payload['length'] ?? 0;
        return (int) $length;
    }

    /**
     * Execute bitfield operations (BITFIELD)
     *
     * @param string $key Bitmap key
     * @param list<array{operation: 'GET'|'SET'|'INCRBY', offset: int, width: int, signed?: bool, value?: int, increment?: int, overflow?: 'WRAP'|'SAT'|'FAIL'}> $operations List of bitfield operations
     * @return list<int> List of result values (one per operation)
     */
    public function bitField(string $key, array $operations): array
    {
        $response = $this->client->execute('bitmap.bitfield', $key, [
            'operations' => $operations,
        ]);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var list<int>|mixed $results */
        $results = $payload['results'] ?? [];
        if (!is_array($results)) {
            return [];
        }

        return array_map('intval', $results);
    }

    /**
     * Retrieve bitmap statistics
     *
     * @return array{total_bitmaps: int, total_bits: int, setbit_count: int, getbit_count: int, bitcount_count: int, bitop_count: int, bitpos_count: int, bitfield_count: int}
     */
    public function stats(): array
    {
        $response = $this->client->execute('bitmap.stats', '', []);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        return [
            'total_bitmaps' => (int) ($payload['total_bitmaps'] ?? 0),
            'total_bits' => (int) ($payload['total_bits'] ?? 0),
            'setbit_count' => (int) ($payload['setbit_count'] ?? 0),
            'getbit_count' => (int) ($payload['getbit_count'] ?? 0),
            'bitcount_count' => (int) ($payload['bitcount_count'] ?? 0),
            'bitop_count' => (int) ($payload['bitop_count'] ?? 0),
            'bitpos_count' => (int) ($payload['bitpos_count'] ?? 0),
            'bitfield_count' => (int) ($payload['bitfield_count'] ?? 0),
        ];
    }
}


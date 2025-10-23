<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * Key-Value Store operations
 */
class KVStore
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Set a key-value pair
     */
    public function set(string $key, mixed $value, ?int $ttl = null): void
    {
        $data = ['value' => $value];

        if ($ttl !== null) {
            $data['ttl'] = $ttl;
        }

        $this->client->execute('kv.set', $key, $data);
    }

    /**
     * Get a value by key
     */
    public function get(string $key): mixed
    {
        $response = $this->client->execute('kv.get', $key);

        return $response['value'] ?? null;
    }

    /**
     * Delete a key
     */
    public function delete(string $key): void
    {
        $this->client->execute('kv.delete', $key);
    }

    /**
     * Check if key exists
     */
    public function exists(string $key): bool
    {
        $response = $this->client->execute('kv.exists', $key);

        return $response['exists'] ?? false;
    }

    /**
     * Increment a numeric value
     */
    public function incr(string $key, int $delta = 1): int
    {
        $response = $this->client->execute('kv.incr', $key, ['delta' => $delta]);

        return (int) ($response['value'] ?? 0);
    }

    /**
     * Decrement a numeric value
     */
    public function decr(string $key, int $delta = 1): int
    {
        $response = $this->client->execute('kv.decr', $key, ['delta' => $delta]);

        return (int) ($response['value'] ?? 0);
    }

    /**
     * Get KV store statistics
     *
     * @return array<string, mixed>
     */
    public function stats(): array
    {
        return $this->client->execute('kv.stats', '*');
    }

    /**
     * Scan keys by prefix
     *
     * @return array<string>
     */
    public function scan(string $prefix, int $limit = 100): array
    {
        $response = $this->client->execute('kv.scan', $prefix, ['limit' => $limit]);

        return $response['keys'] ?? [];
    }
}


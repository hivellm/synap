<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * Hash data structure operations (Redis-compatible)
 */
class HashManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Set field in hash
     *
     * @param string $key Hash key
     * @param string $field Field name
     * @param string|int|float $value Field value
     */
    public function set(string $key, string $field, string|int|float $value): bool
    {
        $response = $this->client->execute('hash.set', $key, [
            'field' => $field,
            'value' => (string) $value,
        ]);

        return (bool) ($response['success'] ?? false);
    }

    /**
     * Get field from hash
     */
    public function get(string $key, string $field): ?string
    {
        $response = $this->client->execute('hash.get', $key, ['field' => $field]);

        return $response['value'] ?? null;
    }

    /**
     * Get all fields and values from hash
     *
     * @return array<string, string>
     */
    public function getAll(string $key): array
    {
        $response = $this->client->execute('hash.getall', $key);

        return $response['fields'] ?? [];
    }

    /**
     * Delete field from hash
     */
    public function delete(string $key, string $field): int
    {
        $response = $this->client->execute('hash.del', $key, ['field' => $field]);

        return (int) ($response['deleted'] ?? 0);
    }

    /**
     * Check if field exists in hash
     */
    public function exists(string $key, string $field): bool
    {
        $response = $this->client->execute('hash.exists', $key, ['field' => $field]);

        return (bool) ($response['exists'] ?? false);
    }

    /**
     * Get all field names in hash
     *
     * @return list<string>
     */
    public function keys(string $key): array
    {
        $response = $this->client->execute('hash.keys', $key);

        return $response['fields'] ?? [];
    }

    /**
     * Get all values in hash
     *
     * @return list<string>
     */
    public function values(string $key): array
    {
        $response = $this->client->execute('hash.values', $key);

        return $response['values'] ?? [];
    }

    /**
     * Get number of fields in hash
     */
    public function len(string $key): int
    {
        $response = $this->client->execute('hash.len', $key);

        return (int) ($response['length'] ?? 0);
    }

    /**
     * Set multiple fields in hash
     *
     * @param array<string, string|int|float> $fields
     */
    public function mset(string $key, array $fields): bool
    {
        $strFields = array_map('strval', $fields);
        $response = $this->client->execute('hash.mset', $key, ['fields' => $strFields]);

        return (bool) ($response['success'] ?? false);
    }

    /**
     * Get multiple fields from hash
     *
     * @param list<string> $fields
     * @return array<string, string|null>
     */
    public function mget(string $key, array $fields): array
    {
        $response = $this->client->execute('hash.mget', $key, ['fields' => $fields]);

        return $response['values'] ?? [];
    }

    /**
     * Increment field value by integer
     */
    public function incrBy(string $key, string $field, int $increment): int
    {
        $response = $this->client->execute('hash.incrby', $key, [
            'field' => $field,
            'increment' => $increment,
        ]);

        return (int) ($response['value'] ?? 0);
    }

    /**
     * Increment field value by float
     */
    public function incrByFloat(string $key, string $field, float $increment): float
    {
        $response = $this->client->execute('hash.incrbyfloat', $key, [
            'field' => $field,
            'increment' => $increment,
        ]);

        return (float) ($response['value'] ?? 0.0);
    }

    /**
     * Set field only if it doesn't exist
     */
    public function setNX(string $key, string $field, string|int|float $value): bool
    {
        $response = $this->client->execute('hash.setnx', $key, [
            'field' => $field,
            'value' => (string) $value,
        ]);

        return (bool) ($response['created'] ?? false);
    }
}


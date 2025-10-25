<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * List data structure operations (Redis-compatible)
 */
class ListManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Push elements to left (head) of list
     *
     * @param list<string> $values
     */
    public function lpush(string $key, array $values): int
    {
        $response = $this->client->execute('list.lpush', $key, ['values' => $values]);

        return (int) ($response['length'] ?? 0);
    }

    /**
     * Push elements to right (tail) of list
     *
     * @param list<string> $values
     */
    public function rpush(string $key, array $values): int
    {
        $response = $this->client->execute('list.rpush', $key, ['values' => $values]);

        return (int) ($response['length'] ?? 0);
    }

    /**
     * Pop elements from left (head) of list
     *
     * @return list<string>
     */
    public function lpop(string $key, int $count = 1): array
    {
        $response = $this->client->execute('list.lpop', $key, ['count' => $count]);

        return $response['values'] ?? [];
    }

    /**
     * Pop elements from right (tail) of list
     *
     * @return list<string>
     */
    public function rpop(string $key, int $count = 1): array
    {
        $response = $this->client->execute('list.rpop', $key, ['count' => $count]);

        return $response['values'] ?? [];
    }

    /**
     * Get range of elements from list
     *
     * @return list<string>
     */
    public function range(string $key, int $start = 0, int $stop = -1): array
    {
        $response = $this->client->execute('list.range', $key, [
            'start' => $start,
            'stop' => $stop,
        ]);

        return $response['values'] ?? [];
    }

    /**
     * Get list length
     */
    public function len(string $key): int
    {
        $response = $this->client->execute('list.len', $key);

        return (int) ($response['length'] ?? 0);
    }

    /**
     * Get element at index
     */
    public function index(string $key, int $index): ?string
    {
        $response = $this->client->execute('list.index', $key, ['index' => $index]);

        return $response['value'] ?? null;
    }

    /**
     * Set element at index
     */
    public function set(string $key, int $index, string $value): bool
    {
        $response = $this->client->execute('list.set', $key, [
            'index' => $index,
            'value' => $value,
        ]);

        return (bool) ($response['success'] ?? false);
    }

    /**
     * Trim list to specified range
     */
    public function trim(string $key, int $start, int $stop): bool
    {
        $response = $this->client->execute('list.trim', $key, [
            'start' => $start,
            'stop' => $stop,
        ]);

        return (bool) ($response['success'] ?? false);
    }
}


<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * Set data structure operations (Redis-compatible)
 */
class SetManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Add members to set
     *
     * @param list<string> $members
     */
    public function add(string $key, array $members): int
    {
        $response = $this->client->execute('set.add', $key, ['members' => $members]);

        return (int) ($response['added'] ?? 0);
    }

    /**
     * Remove members from set
     *
     * @param list<string> $members
     */
    public function rem(string $key, array $members): int
    {
        $response = $this->client->execute('set.rem', $key, ['members' => $members]);

        return (int) ($response['removed'] ?? 0);
    }

    /**
     * Check if member exists in set
     */
    public function isMember(string $key, string $member): bool
    {
        $response = $this->client->execute('set.ismember', $key, ['member' => $member]);

        return (bool) ($response['is_member'] ?? false);
    }

    /**
     * Get all members of set
     *
     * @return list<string>
     */
    public function members(string $key): array
    {
        $response = $this->client->execute('set.members', $key);

        return $response['members'] ?? [];
    }

    /**
     * Get set cardinality (size)
     */
    public function card(string $key): int
    {
        $response = $this->client->execute('set.card', $key);

        return (int) ($response['cardinality'] ?? 0);
    }

    /**
     * Remove and return random members
     *
     * @return list<string>
     */
    public function pop(string $key, int $count = 1): array
    {
        $response = $this->client->execute('set.pop', $key, ['count' => $count]);

        return $response['members'] ?? [];
    }

    /**
     * Get random members without removing
     *
     * @return list<string>
     */
    public function randMember(string $key, int $count = 1): array
    {
        $response = $this->client->execute('set.randmember', $key, ['count' => $count]);

        return $response['members'] ?? [];
    }

    /**
     * Move member from source to destination set
     */
    public function move(string $source, string $destination, string $member): bool
    {
        $response = $this->client->execute('set.move', $source, [
            'destination' => $destination,
            'member' => $member,
        ]);

        return (bool) ($response['moved'] ?? false);
    }

    /**
     * Get intersection of sets
     *
     * @param list<string> $keys
     * @return list<string>
     */
    public function inter(array $keys): array
    {
        $response = $this->client->execute('set.inter', '', ['keys' => $keys]);

        return $response['members'] ?? [];
    }

    /**
     * Get union of sets
     *
     * @param list<string> $keys
     * @return list<string>
     */
    public function union(array $keys): array
    {
        $response = $this->client->execute('set.union', '', ['keys' => $keys]);

        return $response['members'] ?? [];
    }

    /**
     * Get difference of sets
     *
     * @param list<string> $keys
     * @return list<string>
     */
    public function diff(array $keys): array
    {
        $response = $this->client->execute('set.diff', '', ['keys' => $keys]);

        return $response['members'] ?? [];
    }
}


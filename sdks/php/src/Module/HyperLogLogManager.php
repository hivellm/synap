<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * HyperLogLog operations (Redis-compatible)
 */
class HyperLogLogManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Add elements to a HyperLogLog structure (PFADD)
     *
     * @param list<string> $elements Elements to add
     * @return int Number of elements added (approximate)
     */
    public function pfAdd(string $key, array $elements): int
    {
        if (empty($elements)) {
            return 0;
        }

        // Encode elements to byte arrays
        $encoded = array_map(
            fn (string $el) => array_map('ord', str_split($el)),
            $elements
        );

        $response = $this->client->execute('hyperloglog.pfadd', $key, [
            'elements' => $encoded,
        ]);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var int|mixed $added */
        $added = $payload['added'] ?? 0;
        return (int) $added;
    }

    /**
     * Estimate cardinality of a HyperLogLog structure (PFCOUNT)
     *
     * @return int Estimated cardinality (approximate count)
     */
    public function pfCount(string $key): int
    {
        $response = $this->client->execute('hyperloglog.pfcount', $key, []);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var int|mixed $count */
        $count = $payload['count'] ?? 0;
        return (int) $count;
    }

    /**
     * Merge multiple HyperLogLog structures (PFMERGE)
     *
     * @param list<string> $sources Source HyperLogLog keys to merge
     * @return int Estimated cardinality of merged result
     */
    public function pfMerge(string $destination, array $sources): int
    {
        if (empty($sources)) {
            throw new \InvalidArgumentException('PFMERGE requires at least one source key');
        }

        $response = $this->client->execute('hyperloglog.pfmerge', $destination, [
            'sources' => $sources,
        ]);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        /** @var int|mixed $count */
        $count = $payload['count'] ?? 0;
        return (int) $count;
    }

    /**
     * Retrieve HyperLogLog statistics
     *
     * @return array{total_hlls: int, total_cardinality: int, pfadd_count: int, pfcount_count: int, pfmerge_count: int}
     */
    public function stats(): array
    {
        $response = $this->client->execute('hyperloglog.stats', '', []);

        // Extract from payload if present
        $payload = $response['payload'] ?? $response;

        return [
            'total_hlls' => (int) ($payload['total_hlls'] ?? 0),
            'total_cardinality' => (int) ($payload['total_cardinality'] ?? 0),
            'pfadd_count' => (int) ($payload['pfadd_count'] ?? 0),
            'pfcount_count' => (int) ($payload['pfcount_count'] ?? 0),
            'pfmerge_count' => (int) ($payload['pfmerge_count'] ?? 0),
        ];
    }
}


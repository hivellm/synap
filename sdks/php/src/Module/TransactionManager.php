<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * Transaction operations (Redis-compatible)
 */
class TransactionManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Start a transaction (MULTI)
     *
     * @param string|null $clientId Optional client identifier to group commands within the same transaction
     * @return array{success: bool, message: string}
     */
    public function multi(?string $clientId = null): array
    {
        $payload = [];
        if ($clientId !== null) {
            $payload['client_id'] = $clientId;
        }

        $response = $this->client->execute('transaction.multi', '', $payload);
        $payload = $response['payload'] ?? $response;

        return [
            'success' => $payload['success'] ?? true,
            'message' => $payload['message'] ?? 'Transaction started',
        ];
    }

    /**
     * Discard the current transaction (DISCARD)
     *
     * @param string|null $clientId Optional client identifier for the transaction
     * @return array{success: bool, message: string}
     */
    public function discard(?string $clientId = null): array
    {
        $payload = [];
        if ($clientId !== null) {
            $payload['client_id'] = $clientId;
        }

        $response = $this->client->execute('transaction.discard', '', $payload);
        $payload = $response['payload'] ?? $response;

        return [
            'success' => $payload['success'] ?? true,
            'message' => $payload['message'] ?? 'Transaction discarded',
        ];
    }

    /**
     * Watch keys for optimistic locking (WATCH)
     *
     * @param list<string> $keys List of keys to watch for changes
     * @param string|null $clientId Optional client identifier for the transaction
     * @return array{success: bool, message: string}
     * @throws \InvalidArgumentException If keys list is empty
     */
    public function watch(array $keys, ?string $clientId = null): array
    {
        if (empty($keys)) {
            throw new \InvalidArgumentException('Transaction watch requires at least one key');
        }

        $payload = ['keys' => $keys];
        if ($clientId !== null) {
            $payload['client_id'] = $clientId;
        }

        $response = $this->client->execute('transaction.watch', '', $payload);
        $payload = $response['payload'] ?? $response;

        return [
            'success' => $payload['success'] ?? true,
            'message' => $payload['message'] ?? 'Keys watched',
        ];
    }

    /**
     * Remove all watched keys (UNWATCH)
     *
     * @param string|null $clientId Optional client identifier for the transaction
     * @return array{success: bool, message: string}
     */
    public function unwatch(?string $clientId = null): array
    {
        $payload = [];
        if ($clientId !== null) {
            $payload['client_id'] = $clientId;
        }

        $response = $this->client->execute('transaction.unwatch', '', $payload);
        $payload = $response['payload'] ?? $response;

        return [
            'success' => $payload['success'] ?? true,
            'message' => $payload['message'] ?? 'Keys unwatched',
        ];
    }

    /**
     * Execute queued commands (EXEC)
     *
     * @param string|null $clientId Optional client identifier for the transaction
     * @return array{success: true, results: list<mixed>}|array{success: false, aborted: true, message?: string}
     */
    public function exec(?string $clientId = null): array
    {
        $payload = [];
        if ($clientId !== null) {
            $payload['client_id'] = $clientId;
        }

        $response = $this->client->execute('transaction.exec', '', $payload);
        $payload = $response['payload'] ?? $response;

        if (isset($payload['results']) && is_array($payload['results'])) {
            return [
                'success' => true,
                'results' => $payload['results'],
            ];
        }

        return [
            'success' => false,
            'aborted' => true,
            'message' => $payload['message'] ?? null,
        ];
    }
}


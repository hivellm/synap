<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;
use Synap\SDK\Types\QueueMessage;

/**
 * Message Queue operations
 */
class QueueManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Create a new queue
     */
    public function createQueue(string $name, ?int $maxSize = null, ?int $messageTtl = null): void
    {
        $data = [];

        if ($maxSize !== null) {
            $data['max_size'] = $maxSize;
        }

        if ($messageTtl !== null) {
            $data['message_ttl'] = $messageTtl;
        }

        $this->client->execute('queue.create', $name, $data);
    }

    /**
     * Delete a queue
     */
    public function deleteQueue(string $name): void
    {
        $this->client->execute('queue.delete', $name);
    }

    /**
     * Publish a message to queue
     */
    public function publish(
        string $queue,
        mixed $message,
        ?int $priority = null,
        ?int $maxRetries = null
    ): string {
        $data = ['message' => $message];

        if ($priority !== null) {
            $data['priority'] = $priority;
        }

        if ($maxRetries !== null) {
            $data['max_retries'] = $maxRetries;
        }

        $response = $this->client->execute('queue.publish', $queue, $data);

        return $response['message_id'] ?? '';
    }

    /**
     * Consume a message from queue
     */
    public function consume(string $queue, string $consumerId): ?QueueMessage
    {
        $response = $this->client->execute('queue.consume', $queue, [
            'consumer_id' => $consumerId,
        ]);

        if (!isset($response['message'])) {
            return null;
        }

        $msg = $response['message'];

        return new QueueMessage(
            id: $msg['id'] ?? '',
            payload: $msg['payload'] ?? null,
            priority: $msg['priority'] ?? 0,
            retries: $msg['retries'] ?? 0,
            maxRetries: $msg['max_retries'] ?? 3,
            timestamp: $msg['timestamp'] ?? 0
        );
    }

    /**
     * Acknowledge message (success)
     */
    public function ack(string $queue, string $messageId): void
    {
        $this->client->execute('queue.ack', $queue, ['message_id' => $messageId]);
    }

    /**
     * Negative acknowledge (requeue)
     */
    public function nack(string $queue, string $messageId): void
    {
        $this->client->execute('queue.nack', $queue, ['message_id' => $messageId]);
    }

    /**
     * Get queue statistics
     *
     * @return array<string, mixed>
     */
    public function stats(string $queue): array
    {
        return $this->client->execute('queue.stats', $queue);
    }

    /**
     * List all queues
     *
     * @return array<string>
     */
    public function list(): array
    {
        $response = $this->client->execute('queue.list', '*');

        return $response['queues'] ?? [];
    }
}


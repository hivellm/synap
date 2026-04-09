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
        $this->client->sendCommand('queue.create', [
            'name'             => $name,
            'max_depth'        => $maxSize ?? 0,
            'ack_deadline_secs' => $messageTtl ?? 0,
        ]);
    }

    /**
     * Delete a queue
     */
    public function deleteQueue(string $name): void
    {
        $this->client->sendCommand('queue.delete', ['queue' => $name]);
    }

    /**
     * Publish a message to a queue
     */
    public function publish(
        string $queue,
        mixed $message,
        ?int $priority = null,
        ?int $maxRetries = null
    ): string {
        $response = $this->client->sendCommand('queue.publish', [
            'queue'       => $queue,
            'payload'     => $message,
            'priority'    => $priority ?? 0,
            'max_retries' => $maxRetries ?? 3,
        ]);

        $messageId = $response['message_id'] ?? '';
        assert(is_string($messageId));

        return $messageId;
    }

    /**
     * Consume a message from a queue
     */
    public function consume(string $queue, string $consumerId): ?QueueMessage
    {
        $response = $this->client->sendCommand('queue.consume', [
            'queue'       => $queue,
            'consumer_id' => $consumerId,
        ]);

        if (! isset($response['message']) || ! is_array($response['message'])) {
            return null;
        }

        $msg = $response['message'];

        $id         = $msg['id'] ?? '';
        $priority   = $msg['priority'] ?? 0;
        $retries    = $msg['retries'] ?? 0;
        $maxRetries = $msg['max_retries'] ?? 3;
        $timestamp  = $msg['timestamp'] ?? 0;

        assert(is_string($id));
        assert(is_int($priority) || is_numeric($priority));
        assert(is_int($retries) || is_numeric($retries));
        assert(is_int($maxRetries) || is_numeric($maxRetries));
        assert(is_int($timestamp) || is_numeric($timestamp));

        return new QueueMessage(
            id: $id,
            payload: $msg['payload'] ?? null,
            priority: (int) $priority,
            retries: (int) $retries,
            maxRetries: (int) $maxRetries,
            timestamp: (int) $timestamp
        );
    }

    /**
     * Acknowledge message (success)
     */
    public function ack(string $queue, string $messageId): void
    {
        $this->client->sendCommand('queue.ack', [
            'queue'      => $queue,
            'message_id' => $messageId,
        ]);
    }

    /**
     * Negative acknowledge (requeue)
     */
    public function nack(string $queue, string $messageId, int $delaySecs = 0): void
    {
        $this->client->sendCommand('queue.nack', [
            'queue'      => $queue,
            'message_id' => $messageId,
            'delay_secs' => $delaySecs,
        ]);
    }

    /**
     * Get queue statistics
     *
     * @return array<string, mixed>
     */
    public function stats(string $queue): array
    {
        return $this->client->sendCommand('queue.stats', ['queue' => $queue]);
    }

    /**
     * List all queues
     *
     * @return array<string>
     */
    public function list(): array
    {
        $response = $this->client->sendCommand('queue.list', []);
        $queues = $response['queues'] ?? [];

        assert(is_array($queues));

        /** @var array<string> */
        return $queues;
    }
}

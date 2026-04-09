<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * Pub/Sub operations
 */
class PubSubManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Publish a message to a topic.
     *
     * @return int Number of subscribers that received the message
     */
    public function publish(string $topic, mixed $message): int
    {
        $response = $this->client->sendCommand('pubsub.publish', [
            'topic'   => $topic,
            'payload' => $message,
        ]);

        $delivered = $response['subscribers_matched'] ?? 0;
        assert(is_int($delivered) || is_numeric($delivered));

        return (int) $delivered;
    }

    /**
     * Register a subscription on the server (HTTP transport only).
     *
     * For real-time message delivery on SynapRPC, use :meth:`observe` instead.
     *
     * @param array<string> $topics Topic patterns (supports wildcards like user.*)
     */
    public function subscribeTopics(string $subscriberId, array $topics): void
    {
        $this->client->sendCommand('pubsub.subscribe', [
            'subscriber_id' => $subscriberId,
            'topics'        => $topics,
        ]);
    }

    /**
     * Unsubscribe from topics.
     *
     * @param array<string> $topics
     */
    public function unsubscribeTopics(string $subscriberId, array $topics): void
    {
        $this->client->sendCommand('pubsub.unsubscribe', [
            'subscriber_id' => $subscriberId,
            'topics'        => $topics,
        ]);
    }

    /**
     * List active topics.
     *
     * @return array<string>
     */
    public function listTopics(): array
    {
        $response = $this->client->sendCommand('pubsub.topics', []);
        $topics = $response['topics'] ?? [];

        assert(is_array($topics));

        /** @var array<string> */
        return $topics;
    }

    /**
     * Get Pub/Sub statistics.
     *
     * @return array<string, mixed>
     */
    public function stats(): array
    {
        return $this->client->sendCommand('pubsub.stats', []);
    }

    /**
     * Subscribe and invoke callback for each push message (SynapRPC only).
     *
     * Opens a dedicated server-push TCP connection and blocks the current
     * PHP process, calling $onMessage for each received message.
     * On non-SynapRPC transports, registers the subscription via HTTP and
     * returns immediately (no real-time delivery).
     *
     * @param array<string> $topics     Topic patterns to subscribe to
     * @param callable      $onMessage  Invoked with each push-message array
     * @param callable|null $shouldStop Optional predicate; loop exits when it returns true
     */
    public function observe(
        array $topics,
        callable $onMessage,
        ?callable $shouldStop = null,
        ?string $subscriberId = null,
    ): void {
        $rpc = $this->client->getSynapRpcTransport();

        if ($rpc !== null) {
            $rpc->subscribePush($topics, $onMessage, $shouldStop);
        } else {
            // HTTP fallback — register subscription, no real-time delivery
            $sid = $subscriberId ?? ('php-sub-' . time());
            $this->subscribeTopics($sid, $topics);
        }
    }
}

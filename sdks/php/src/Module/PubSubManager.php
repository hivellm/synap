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
     * Publish a message to topic
     */
    public function publish(
        string $topic,
        mixed $message,
        ?int $priority = null,
        ?array $headers = null
    ): int {
        $data = ['message' => $message];

        if ($priority !== null) {
            $data['priority'] = $priority;
        }

        if ($headers !== null) {
            $data['headers'] = $headers;
        }

        $response = $this->client->execute('pubsub.publish', $topic, $data);

        return (int) ($response['delivered'] ?? 0);
    }

    /**
     * Subscribe to topics
     *
     * @param array<string> $topics
     */
    public function subscribeTopics(string $subscriberId, array $topics): string
    {
        $response = $this->client->execute('pubsub.subscribe', $subscriberId, [
            'topics' => $topics,
        ]);

        return $response['subscription_id'] ?? '';
    }

    /**
     * Unsubscribe from topics
     *
     * @param array<string> $topics
     */
    public function unsubscribe(string $subscriberId, array $topics): void
    {
        $this->client->execute('pubsub.unsubscribe', $subscriberId, [
            'topics' => $topics,
        ]);
    }

    /**
     * List active topics
     *
     * @return array<string>
     */
    public function listTopics(): array
    {
        $response = $this->client->execute('pubsub.list_topics', '*');

        return $response['topics'] ?? [];
    }

    /**
     * Get subscriber information
     *
     * @return array<string, mixed>
     */
    public function getSubscriber(string $subscriberId): array
    {
        return $this->client->execute('pubsub.get_subscriber', $subscriberId);
    }
}


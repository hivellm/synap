<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;
use Synap\SDK\Types\StreamEvent;

/**
 * Event Stream operations
 */
class StreamManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Create a stream room
     */
    public function createRoom(string $room, ?int $maxEvents = null): void
    {
        $data = [];

        if ($maxEvents !== null) {
            $data['max_events'] = $maxEvents;
        }

        $this->client->execute('stream.create', $room, $data);
    }

    /**
     * Delete a stream room
     */
    public function deleteRoom(string $room): void
    {
        $this->client->execute('stream.delete', $room);
    }

    /**
     * Publish an event to stream
     */
    public function publish(string $room, string $event, mixed $data): int
    {
        $response = $this->client->execute('stream.publish', $room, [
            'event' => $event,
            'data' => $data,
        ]);

        return (int) ($response['offset'] ?? 0);
    }

    /**
     * Consume events from stream
     *
     * @return array<StreamEvent>
     */
    public function consume(string $room, int $offset = 0, int $limit = 100): array
    {
        $response = $this->client->execute('stream.consume', $room, [
            'offset' => $offset,
            'limit' => $limit,
        ]);

        $events = $response['events'] ?? [];
        $result = [];

        foreach ($events as $event) {
            $result[] = new StreamEvent(
                offset: $event['offset'] ?? 0,
                event: $event['event'] ?? '',
                data: $event['data'] ?? null,
                timestamp: $event['timestamp'] ?? 0
            );
        }

        return $result;
    }

    /**
     * Get stream statistics
     *
     * @return array<string, mixed>
     */
    public function stats(string $room): array
    {
        return $this->client->execute('stream.stats', $room);
    }

    /**
     * List all stream rooms
     *
     * @return array<string>
     */
    public function list(): array
    {
        $response = $this->client->execute('stream.list', '*');

        return $response['rooms'] ?? [];
    }
}


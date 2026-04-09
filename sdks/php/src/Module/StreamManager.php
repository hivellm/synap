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
    public function createRoom(string $room): void
    {
        $this->client->sendCommand('stream.create', ['room' => $room]);
    }

    /**
     * Delete a stream room
     */
    public function deleteRoom(string $room): void
    {
        $this->client->sendCommand('stream.delete', ['room' => $room]);
    }

    /**
     * Publish an event to stream
     */
    public function publish(string $room, string $event, mixed $data): int
    {
        $response = $this->client->sendCommand('stream.publish', [
            'room'  => $room,
            'event' => $event,
            'data'  => $data,
        ]);

        $offset = $response['offset'] ?? 0;
        assert(is_int($offset) || is_numeric($offset));

        return (int) $offset;
    }

    /**
     * Read events from stream
     *
     * @return array<StreamEvent>
     */
    public function read(string $room, int $offset = 0, string $subscriberId = 'sdk-reader'): array
    {
        $response = $this->client->sendCommand('stream.consume', [
            'room'          => $room,
            'subscriber_id' => $subscriberId,
            'from_offset'   => $offset,
        ]);

        $events = $response['events'] ?? [];

        if (! is_array($events)) {
            return [];
        }

        $result = [];

        foreach ($events as $event) {
            if (! is_array($event)) {
                continue;
            }

            $eventOffset    = $event['offset'] ?? 0;
            $eventName      = $event['event'] ?? '';
            $eventTimestamp = $event['timestamp'] ?? 0;
            $eventRoom      = $event['room'] ?? $room;

            assert(is_int($eventOffset) || is_numeric($eventOffset));
            assert(is_string($eventName));
            assert(is_int($eventTimestamp) || is_numeric($eventTimestamp));

            $result[] = new StreamEvent(
                offset: (int) $eventOffset,
                event: $eventName,
                data: $event['data'] ?? null,
                timestamp: (int) $eventTimestamp
            );
        }

        return $result;
    }

    /**
     * List all stream rooms
     *
     * @return array<string>
     */
    public function listRooms(): array
    {
        $response = $this->client->sendCommand('stream.list', []);
        $rooms = $response['rooms'] ?? [];

        assert(is_array($rooms));

        /** @var array<string> */
        return $rooms;
    }

    /**
     * Get stream statistics
     *
     * @return array<string, mixed>
     */
    public function stats(string $room): array
    {
        return $this->client->sendCommand('stream.stats', ['room' => $room]);
    }
}

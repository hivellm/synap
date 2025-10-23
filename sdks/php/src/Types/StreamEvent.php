<?php

declare(strict_types=1);

namespace Synap\SDK\Types;

/**
 * Represents an event in a stream
 */
readonly class StreamEvent
{
    public function __construct(
        public int $offset,
        public string $event,
        public mixed $data,
        public int $timestamp = 0
    ) {
    }

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        return [
            'offset' => $this->offset,
            'event' => $this->event,
            'data' => $this->data,
            'timestamp' => $this->timestamp,
        ];
    }
}


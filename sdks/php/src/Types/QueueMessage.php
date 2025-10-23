<?php

declare(strict_types=1);

namespace Synap\SDK\Types;

/**
 * Represents a message in a queue
 */
readonly class QueueMessage
{
    public function __construct(
        public string $id,
        public mixed $payload,
        public int $priority = 0,
        public int $retries = 0,
        public int $maxRetries = 3,
        public int $timestamp = 0
    ) {
    }

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        return [
            'id' => $this->id,
            'payload' => $this->payload,
            'priority' => $this->priority,
            'retries' => $this->retries,
            'max_retries' => $this->maxRetries,
            'timestamp' => $this->timestamp,
        ];
    }
}


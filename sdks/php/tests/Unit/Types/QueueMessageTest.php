<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Types;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Types\QueueMessage;

final class QueueMessageTest extends TestCase
{
    public function testQueueMessageCreation(): void
    {
        $message = new QueueMessage(
            id: 'msg-123',
            payload: ['task' => 'process-video'],
            priority: 9,
            retries: 1,
            maxRetries: 3,
            timestamp: 1234567890
        );

        $this->assertSame('msg-123', $message->id);
        $this->assertSame(['task' => 'process-video'], $message->payload);
        $this->assertSame(9, $message->priority);
        $this->assertSame(1, $message->retries);
        $this->assertSame(3, $message->maxRetries);
        $this->assertSame(1234567890, $message->timestamp);
    }

    public function testToArray(): void
    {
        $message = new QueueMessage(
            id: 'msg-123',
            payload: ['data' => 'test'],
            priority: 5
        );

        $array = $message->toArray();

        $this->assertSame([
            'id' => 'msg-123',
            'payload' => ['data' => 'test'],
            'priority' => 5,
            'retries' => 0,
            'max_retries' => 3,
            'timestamp' => 0,
        ], $array);
    }
}

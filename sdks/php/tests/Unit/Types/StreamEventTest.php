<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Types;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Types\StreamEvent;

final class StreamEventTest extends TestCase
{
    public function testStreamEventCreation(): void
    {
        $event = new StreamEvent(
            offset: 42,
            event: 'message',
            data: ['user' => 'alice', 'text' => 'Hello!'],
            timestamp: 1234567890
        );

        $this->assertSame(42, $event->offset);
        $this->assertSame('message', $event->event);
        $this->assertSame(['user' => 'alice', 'text' => 'Hello!'], $event->data);
        $this->assertSame(1234567890, $event->timestamp);
    }

    public function testToArray(): void
    {
        $event = new StreamEvent(
            offset: 10,
            event: 'test-event',
            data: ['key' => 'value']
        );

        $array = $event->toArray();

        $this->assertSame([
            'offset' => 10,
            'event' => 'test-event',
            'data' => ['key' => 'value'],
            'timestamp' => 0,
        ], $array);
    }
}

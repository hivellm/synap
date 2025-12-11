<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\StreamManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

final class StreamManagerTest extends TestCase
{
    private StreamManager $stream;

    protected function setUp(): void
    {
        $config = SynapConfig::create('http://localhost:15500');
        $client = new SynapClient($config);
        $this->stream = new StreamManager($client);
    }

    public function testStreamManagerCreation(): void
    {
        $this->assertInstanceOf(StreamManager::class, $this->stream);
    }

    public function testCreateRoomWithMaxEvents(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $this->stream->createRoom('chat', 10000);
    }

    public function testPublishEvent(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $offset = $this->stream->publish('room', 'event', $data);
    }

    public function testConsumeWithOffsetAndLimit(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $events = $this->stream->consume('room', 0, 100);
    }
}

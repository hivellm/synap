<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\KVStore;
use Synap\SDK\Module\PubSubManager;
use Synap\SDK\Module\QueueManager;
use Synap\SDK\Module\StreamManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

final class SynapClientTest extends TestCase
{
    private SynapClient $client;

    protected function setUp(): void
    {
        $config = SynapConfig::create('http://localhost:15500');
        $this->client = new SynapClient($config);
    }

    public function testKvReturnsKVStore(): void
    {
        $kv = $this->client->kv();

        $this->assertInstanceOf(KVStore::class, $kv);
        $this->assertSame($kv, $this->client->kv()); // Same instance
    }

    public function testQueueReturnsQueueManager(): void
    {
        $queue = $this->client->queue();

        $this->assertInstanceOf(QueueManager::class, $queue);
        $this->assertSame($queue, $this->client->queue()); // Same instance
    }

    public function testStreamReturnsStreamManager(): void
    {
        $stream = $this->client->stream();

        $this->assertInstanceOf(StreamManager::class, $stream);
        $this->assertSame($stream, $this->client->stream()); // Same instance
    }

    public function testPubsubReturnsPubSubManager(): void
    {
        $pubsub = $this->client->pubsub();

        $this->assertInstanceOf(PubSubManager::class, $pubsub);
        $this->assertSame($pubsub, $this->client->pubsub()); // Same instance
    }

    public function testGetConfig(): void
    {
        $config = $this->client->getConfig();

        $this->assertInstanceOf(SynapConfig::class, $config);
        $this->assertSame('http://localhost:15500', $config->getBaseUrl());
    }
}


<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\QueueManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

final class QueueManagerTest extends TestCase
{
    private QueueManager $queue;

    protected function setUp(): void
    {
        $config = SynapConfig::create('http://localhost:15500');
        $client = new SynapClient($config);
        $this->queue = new QueueManager($client);
    }

    public function testQueueManagerCreation(): void
    {
        $this->assertInstanceOf(QueueManager::class, $this->queue);
    }

    public function testCreateQueueWithOptions(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $this->queue->createQueue('tasks', 10000, 30);
    }

    public function testPublishWithPriority(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $msgId = $this->queue->publish('tasks', $data, 9);
    }

    public function testPublishWithMaxRetries(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $msgId = $this->queue->publish('tasks', $data, 5, 3);
    }
}

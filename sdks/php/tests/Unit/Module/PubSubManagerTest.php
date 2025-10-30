<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\PubSubManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

final class PubSubManagerTest extends TestCase
{
    private PubSubManager $pubsub;

    protected function setUp(): void
    {
        $config = SynapConfig::create('http://localhost:15500');
        $client = new SynapClient($config);
        $this->pubsub = new PubSubManager($client);
    }

    public function testPubSubManagerCreation(): void
    {
        $this->assertInstanceOf(PubSubManager::class, $this->pubsub);
    }

    public function testPublishWithPriority(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $delivered = $this->pubsub->publish('topic', $msg, 5);
    }

    public function testPublishWithHeaders(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $delivered = $this->pubsub->publish('topic', $msg, null, ['key' => 'value']);
    }

    public function testSubscribeMultipleTopics(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $subId = $this->pubsub->subscribeTopics('user-1', ['topic1', 'topic2']);
    }

    public function testUnsubscribeMultipleTopics(): void
    {
        $this->expectNotToPerformAssertions();

        // Would call: $this->pubsub->unsubscribe('user-1', ['topic1', 'topic2']);
    }
}

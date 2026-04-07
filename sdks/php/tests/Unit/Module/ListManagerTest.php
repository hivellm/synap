<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\ListManager;
use Synap\SDK\SynapClient;

final class ListManagerTest extends TestCase
{
    public function testLPushReturnsLength(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['length' => 3]);

        $list = new ListManager($client);
        $result = $list->lpush('tasks', ['task1', 'task2']);

        $this->assertSame(3, $result);
    }

    public function testRPushReturnsLength(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['length' => 2]);

        $list = new ListManager($client);
        $result = $list->rpush('tasks', ['task1']);

        $this->assertSame(2, $result);
    }

    public function testLPopReturnsValues(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['values' => ['task1']]);

        $list = new ListManager($client);
        $result = $list->lpop('tasks');

        $this->assertCount(1, $result);
        $this->assertSame('task1', $result[0]);
    }

    public function testRangeReturnsValues(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['values' => ['task1', 'task2', 'task3']]);

        $list = new ListManager($client);
        $result = $list->range('tasks');

        $this->assertCount(3, $result);
    }

    public function testLenReturnsLength(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['length' => 5]);

        $list = new ListManager($client);
        $result = $list->len('tasks');

        $this->assertSame(5, $result);
    }

    public function testIndexReturnsValue(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['value' => 'task2']);

        $list = new ListManager($client);
        $result = $list->index('tasks', 1);

        $this->assertSame('task2', $result);
    }
}


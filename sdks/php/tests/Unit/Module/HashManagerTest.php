<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\HashManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

final class HashManagerTest extends TestCase
{
    public function testSetReturnsTrue(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['success' => true]);

        $hash = new HashManager($client);
        $result = $hash->set('user:1', 'name', 'Alice');

        $this->assertTrue($result);
    }

    public function testGetReturnsValue(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['value' => 'Alice']);

        $hash = new HashManager($client);
        $result = $hash->get('user:1', 'name');

        $this->assertSame('Alice', $result);
    }

    public function testGetAllReturnsFields(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['fields' => ['name' => 'Alice', 'age' => '30']]);

        $hash = new HashManager($client);
        $result = $hash->getAll('user:1');

        $this->assertCount(2, $result);
        $this->assertSame('Alice', $result['name']);
    }

    public function testDeleteReturnsCount(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['deleted' => 1]);

        $hash = new HashManager($client);
        $result = $hash->delete('user:1', 'name');

        $this->assertSame(1, $result);
    }

    public function testExistsReturnsTrue(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['exists' => true]);

        $hash = new HashManager($client);
        $result = $hash->exists('user:1', 'name');

        $this->assertTrue($result);
    }

    public function testLenReturnsCount(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['length' => 2]);

        $hash = new HashManager($client);
        $result = $hash->len('user:1');

        $this->assertSame(2, $result);
    }

    public function testIncrByReturnsValue(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['value' => 5]);

        $hash = new HashManager($client);
        $result = $hash->incrBy('counters', 'visits', 1);

        $this->assertSame(5, $result);
    }
}


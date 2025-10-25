<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\SetManager;
use Synap\SDK\SynapClient;

final class SetManagerTest extends TestCase
{
    public function testAddReturnsCount(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['added' => 3]);

        $set = new SetManager($client);
        $result = $set->add('tags', ['python', 'redis', 'typescript']);

        $this->assertSame(3, $result);
    }

    public function testRemReturnsCount(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['removed' => 1]);

        $set = new SetManager($client);
        $result = $set->rem('tags', ['typescript']);

        $this->assertSame(1, $result);
    }

    public function testIsMemberReturnsTrue(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['is_member' => true]);

        $set = new SetManager($client);
        $result = $set->isMember('tags', 'python');

        $this->assertTrue($result);
    }

    public function testMembersReturnsArray(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['members' => ['python', 'redis']]);

        $set = new SetManager($client);
        $result = $set->members('tags');

        $this->assertCount(2, $result);
    }

    public function testCardReturnsCount(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['cardinality' => 3]);

        $set = new SetManager($client);
        $result = $set->card('tags');

        $this->assertSame(3, $result);
    }

    public function testPopReturnsMembers(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['members' => ['python']]);

        $set = new SetManager($client);
        $result = $set->pop('tags', 1);

        $this->assertCount(1, $result);
    }

    public function testInterReturnsIntersection(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['members' => ['python']]);

        $set = new SetManager($client);
        $result = $set->inter(['tags1', 'tags2']);

        $this->assertCount(1, $result);
        $this->assertSame('python', $result[0]);
    }

    public function testUnionReturnsUnion(): void
    {
        $client = $this->createMock(SynapClient::class);
        $client->expects($this->once())
            ->method('execute')
            ->willReturn(['members' => ['python', 'redis', 'typescript']]);

        $set = new SetManager($client);
        $result = $set->union(['tags1', 'tags2']);

        $this->assertCount(3, $result);
    }
}


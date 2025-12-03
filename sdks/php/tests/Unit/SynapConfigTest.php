<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Exception\SynapException;
use Synap\SDK\SynapConfig;

final class SynapConfigTest extends TestCase
{
    public function testConfigCreation(): void
    {
        $config = SynapConfig::create('http://localhost:15500');

        $this->assertSame('http://localhost:15500', $config->getBaseUrl());
        $this->assertSame(30, $config->getTimeout());
        $this->assertNull($config->getAuthToken());
        $this->assertSame(3, $config->getMaxRetries());
    }

    public function testWithTimeout(): void
    {
        $config = SynapConfig::create('http://localhost:15500')
            ->withTimeout(60);

        $this->assertSame(60, $config->getTimeout());
    }

    public function testWithAuthToken(): void
    {
        $config = SynapConfig::create('http://localhost:15500')
            ->withAuthToken('test-token');

        $this->assertSame('test-token', $config->getAuthToken());
    }

    public function testWithMaxRetries(): void
    {
        $config = SynapConfig::create('http://localhost:15500')
            ->withMaxRetries(5);

        $this->assertSame(5, $config->getMaxRetries());
    }

    public function testImmutability(): void
    {
        $config1 = SynapConfig::create('http://localhost:15500');
        $config2 = $config1->withTimeout(60);

        $this->assertNotSame($config1, $config2);
        $this->assertSame(30, $config1->getTimeout());
        $this->assertSame(60, $config2->getTimeout());
    }

    public function testEmptyBaseUrlThrowsException(): void
    {
        $this->expectException(SynapException::class);
        $this->expectExceptionMessage('Base URL cannot be empty');

        new SynapConfig('');
    }

    public function testBaseUrlTrimsTrailingSlash(): void
    {
        $config = SynapConfig::create('http://localhost:15500/');

        $this->assertSame('http://localhost:15500', $config->getBaseUrl());
    }
}

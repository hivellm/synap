<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Exception;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Exception\SynapException;

final class SynapExceptionTest extends TestCase
{
    public function testHttpError(): void
    {
        $exception = SynapException::httpError('Not Found', 404);

        $this->assertInstanceOf(SynapException::class, $exception);
        $this->assertSame('HTTP Error (404): Not Found', $exception->getMessage());
        $this->assertSame(404, $exception->getCode());
    }

    public function testServerError(): void
    {
        $exception = SynapException::serverError('Internal error');

        $this->assertInstanceOf(SynapException::class, $exception);
        $this->assertSame('Server Error: Internal error', $exception->getMessage());
    }

    public function testNetworkError(): void
    {
        $exception = SynapException::networkError('Connection timeout');

        $this->assertInstanceOf(SynapException::class, $exception);
        $this->assertSame('Network Error: Connection timeout', $exception->getMessage());
    }

    public function testInvalidResponse(): void
    {
        $exception = SynapException::invalidResponse('Malformed JSON');

        $this->assertInstanceOf(SynapException::class, $exception);
        $this->assertSame('Invalid Response: Malformed JSON', $exception->getMessage());
    }

    public function testInvalidConfig(): void
    {
        $exception = SynapException::invalidConfig('Missing URL');

        $this->assertInstanceOf(SynapException::class, $exception);
        $this->assertSame('Invalid Configuration: Missing URL', $exception->getMessage());
    }
}

<?php

declare(strict_types=1);

namespace Synap\SDK\Exception;

use Exception;

/**
 * Base exception for all Synap SDK errors
 */
class SynapException extends Exception
{
    public static function httpError(string $message, int $statusCode): self
    {
        return new self("HTTP Error ({$statusCode}): {$message}", $statusCode);
    }

    public static function serverError(string $message): self
    {
        return new self("Server Error: {$message}");
    }

    public static function networkError(string $message): self
    {
        return new self("Network Error: {$message}");
    }

    public static function invalidResponse(string $message): self
    {
        return new self("Invalid Response: {$message}");
    }

    public static function invalidConfig(string $message): self
    {
        return new self("Invalid Configuration: {$message}");
    }
}

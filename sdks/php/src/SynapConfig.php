<?php

declare(strict_types=1);

namespace Synap\SDK;

use Synap\SDK\Exception\SynapException;

/**
 * Configuration for Synap client
 */
class SynapConfig
{
    private string $baseUrl;
    private int $timeout = 30;
    private ?string $authToken = null;
    private int $maxRetries = 3;

    public function __construct(string $baseUrl)
    {
        if (empty($baseUrl)) {
            throw SynapException::invalidConfig('Base URL cannot be empty');
        }

        $this->baseUrl = rtrim($baseUrl, '/');
    }

    public static function create(string $baseUrl): self
    {
        return new self($baseUrl);
    }

    public function withTimeout(int $timeout): self
    {
        $clone = clone $this;
        $clone->timeout = $timeout;

        return $clone;
    }

    public function withAuthToken(string $token): self
    {
        $clone = clone $this;
        $clone->authToken = $token;

        return $clone;
    }

    public function withMaxRetries(int $retries): self
    {
        $clone = clone $this;
        $clone->maxRetries = $retries;

        return $clone;
    }

    public function getBaseUrl(): string
    {
        return $this->baseUrl;
    }

    public function getTimeout(): int
    {
        return $this->timeout;
    }

    public function getAuthToken(): ?string
    {
        return $this->authToken;
    }

    public function getMaxRetries(): int
    {
        return $this->maxRetries;
    }
}


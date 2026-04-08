<?php

declare(strict_types=1);

namespace Synap\SDK;

use Synap\SDK\Exception\SynapException;
use Synap\SDK\TransportMode;

/**
 * Configuration for Synap client
 */
class SynapConfig
{
    private string $baseUrl;
    private int $timeout = 30;
    private ?string $authToken = null;
    private ?string $username = null;
    private ?string $password = null;
    private int $maxRetries = 3;
    private string $transport = TransportMode::SYNAP_RPC;
    private string $rpcHost = '127.0.0.1';
    private int $rpcPort = 15501;
    private string $resp3Host = '127.0.0.1';
    private int $resp3Port = 6379;

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
        $clone->username = null;
        $clone->password = null;

        return $clone;
    }

    public function withBasicAuth(string $username, string $password): self
    {
        $clone = clone $this;
        $clone->username = $username;
        $clone->password = $password;
        $clone->authToken = null;

        return $clone;
    }

    public function withMaxRetries(int $retries): self
    {
        $clone = clone $this;
        $clone->maxRetries = $retries;

        return $clone;
    }

    public function withHttpTransport(): self
    {
        $clone = clone $this;
        $clone->transport = TransportMode::HTTP;

        return $clone;
    }

    public function withSynapRpcTransport(): self
    {
        $clone = clone $this;
        $clone->transport = TransportMode::SYNAP_RPC;

        return $clone;
    }

    public function withResp3Transport(): self
    {
        $clone = clone $this;
        $clone->transport = TransportMode::RESP3;

        return $clone;
    }

    public function withRpcAddr(string $host, int $port): self
    {
        $clone = clone $this;
        $clone->rpcHost = $host;
        $clone->rpcPort = $port;

        return $clone;
    }

    public function withResp3Addr(string $host, int $port): self
    {
        $clone = clone $this;
        $clone->resp3Host = $host;
        $clone->resp3Port = $port;

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

    public function getUsername(): ?string
    {
        return $this->username;
    }

    public function getPassword(): ?string
    {
        return $this->password;
    }

    public function getMaxRetries(): int
    {
        return $this->maxRetries;
    }

    public function getTransport(): string
    {
        return $this->transport;
    }

    public function getRpcHost(): string
    {
        return $this->rpcHost;
    }

    public function getRpcPort(): int
    {
        return $this->rpcPort;
    }

    public function getResp3Host(): string
    {
        return $this->resp3Host;
    }

    public function getResp3Port(): int
    {
        return $this->resp3Port;
    }
}

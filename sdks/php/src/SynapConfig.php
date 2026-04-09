<?php

declare(strict_types=1);

namespace Synap\SDK;

use Synap\SDK\Exception\SynapException;
use Synap\SDK\TransportMode;

/**
 * Configuration for Synap client.
 *
 * Preferred usage — URL schemes:
 *   new SynapConfig('http://localhost:15500')
 *   new SynapConfig('synap://localhost:15501')
 *   new SynapConfig('resp3://localhost:6379')
 */
class SynapConfig
{
    private string $baseUrl;
    private int $timeout = 30;
    private ?string $authToken = null;
    private ?string $username = null;
    private ?string $password = null;
    private int $maxRetries = 3;
    private string $transport;
    private string $rpcHost = '127.0.0.1';
    private int $rpcPort = 15501;
    private string $resp3Host = '127.0.0.1';
    private int $resp3Port = 6379;

    public function __construct(string $baseUrl)
    {
        if (empty($baseUrl)) {
            throw SynapException::invalidConfig('Base URL cannot be empty');
        }

        if (str_starts_with($baseUrl, 'synap://')) {
            [$host, $port] = self::parseHostPort(substr($baseUrl, strlen('synap://')), 15_501);
            $this->baseUrl    = "http://{$host}:15500";
            $this->transport  = TransportMode::SYNAP_RPC;
            $this->rpcHost    = $host;
            $this->rpcPort    = $port;
        } elseif (str_starts_with($baseUrl, 'resp3://')) {
            [$host, $port] = self::parseHostPort(substr($baseUrl, strlen('resp3://')), 6_379);
            $this->baseUrl   = "http://{$host}:15500";
            $this->transport = TransportMode::RESP3;
            $this->resp3Host = $host;
            $this->resp3Port = $port;
        } else {
            $this->baseUrl   = rtrim($baseUrl, '/');
            $this->transport = TransportMode::HTTP;
        }
    }

    public static function create(string $baseUrl): self
    {
        return new self($baseUrl);
    }

    // ── Immutable builder helpers ──────────────────────────────────────────────

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
        $clone->username  = null;
        $clone->password  = null;

        return $clone;
    }

    public function withBasicAuth(string $username, string $password): self
    {
        $clone = clone $this;
        $clone->username  = $username;
        $clone->password  = $password;
        $clone->authToken = null;

        return $clone;
    }

    public function withMaxRetries(int $retries): self
    {
        $clone = clone $this;
        $clone->maxRetries = $retries;

        return $clone;
    }

    /**
     * @deprecated Use new SynapConfig('http://...') instead.
     */
    public function withHttpTransport(): self
    {
        @trigger_error(
            'withHttpTransport() is deprecated. Pass an http:// URL to the constructor instead.',
            \E_USER_DEPRECATED
        );
        $clone = clone $this;
        $clone->transport = TransportMode::HTTP;

        return $clone;
    }

    /**
     * @deprecated Use new SynapConfig('synap://host:port') instead.
     */
    public function withSynapRpcTransport(): self
    {
        @trigger_error(
            'withSynapRpcTransport() is deprecated. Pass a synap:// URL to the constructor instead.',
            \E_USER_DEPRECATED
        );
        $clone = clone $this;
        $clone->transport = TransportMode::SYNAP_RPC;

        return $clone;
    }

    /**
     * @deprecated Use new SynapConfig('resp3://host:port') instead.
     */
    public function withResp3Transport(): self
    {
        @trigger_error(
            'withResp3Transport() is deprecated. Pass a resp3:// URL to the constructor instead.',
            \E_USER_DEPRECATED
        );
        $clone = clone $this;
        $clone->transport = TransportMode::RESP3;

        return $clone;
    }

    /**
     * @deprecated Use new SynapConfig('synap://host:port') instead.
     */
    public function withRpcAddr(string $host, int $port): self
    {
        @trigger_error(
            'withRpcAddr() is deprecated. Pass a synap://host:port URL to the constructor instead.',
            \E_USER_DEPRECATED
        );
        $clone = clone $this;
        $clone->rpcHost = $host;
        $clone->rpcPort = $port;

        return $clone;
    }

    /**
     * @deprecated Use new SynapConfig('resp3://host:port') instead.
     */
    public function withResp3Addr(string $host, int $port): self
    {
        @trigger_error(
            'withResp3Addr() is deprecated. Pass a resp3://host:port URL to the constructor instead.',
            \E_USER_DEPRECATED
        );
        $clone = clone $this;
        $clone->resp3Host = $host;
        $clone->resp3Port = $port;

        return $clone;
    }

    // ── Getters ────────────────────────────────────────────────────────────────

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

    // ── Internal helpers ───────────────────────────────────────────────────────

    /**
     * Parse "host:port" or "host" with a default port.
     *
     * @return array{0: string, 1: int}
     */
    private static function parseHostPort(string $hostPort, int $defaultPort): array
    {
        $lastColon = strrpos($hostPort, ':');
        if ($lastColon !== false) {
            $host = substr($hostPort, 0, $lastColon);
            $port = (int) substr($hostPort, $lastColon + 1);
            if ($port <= 0 || $port > 65535) {
                $port = $defaultPort;
            }
        } else {
            $host = $hostPort;
            $port = $defaultPort;
        }

        $host = $host !== '' ? $host : '127.0.0.1';

        return [$host, $port];
    }
}

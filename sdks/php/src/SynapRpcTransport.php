<?php

declare(strict_types=1);

namespace Synap\SDK;

use MessagePack\MessagePack;
use Synap\SDK\Exception\SynapException;

/**
 * Blocking TCP transport using the SynapRPC MessagePack binary protocol.
 *
 * Wire format (per frame):
 *   - 4-byte unsigned little-endian length prefix
 *   - MessagePack body
 *
 * Request body: [id: uint, command: str, args: WireValue[]]
 * Response body: [id: uint, result: {"Ok": WireValue} | {"Err": str}]
 *
 * SynapValue is externally-tagged:
 *   "Null"           → null
 *   {"Str": "x"}     → string
 *   {"Int": 42}      → int
 *   {"Float": 3.14}  → float
 *   {"Bool": true}   → bool
 *   {"Bytes": ...}   → binary
 *   {"Array": [...]} → list
 *   {"Map": [[k,v]]} → assoc map
 */
class SynapRpcTransport
{
    /** @var resource|null */
    private mixed $socket = null;

    private int $nextId = 1;

    public function __construct(
        private readonly string $host,
        private readonly int $port,
        private readonly int $timeoutSecs,
    ) {}

    /**
     * Ensure the socket is connected, (re)connecting on demand.
     *
     * @return resource
     */
    private function ensureConnected(): mixed
    {
        if ($this->socket !== null) {
            return $this->socket;
        }

        $errno = 0;
        $errstr = '';
        $sock = @fsockopen($this->host, $this->port, $errno, $errstr, $this->timeoutSecs);
        if ($sock === false) {
            throw SynapException::networkError(
                "SynapRPC connect to {$this->host}:{$this->port} failed ({$errno}): {$errstr}"
            );
        }
        stream_set_timeout($sock, $this->timeoutSecs);
        $this->socket = $sock;

        return $sock;
    }

    /**
     * Execute a command over SynapRPC and return the decoded response value.
     *
     * Automatically reconnects once on network failure.
     *
     * @param string      $cmd  Wire command name (e.g. "GET", "SET")
     * @param list<mixed> $args Command arguments — each will be wrapped as a WireValue
     * @return mixed Decoded PHP value from the server response
     * @throws SynapException on network or server error
     */
    public function execute(string $cmd, array $args): mixed
    {
        try {
            return $this->doExecute($cmd, $args);
        } catch (SynapException $e) {
            // On network error, attempt a single reconnect and retry.
            if (str_contains($e->getMessage(), 'Network Error')) {
                $this->socket = null;
                return $this->doExecute($cmd, $args);
            }
            throw $e;
        }
    }

    /**
     * Internal execute — no retry logic.
     *
     * @param string      $cmd
     * @param list<mixed> $args
     * @return mixed
     */
    private function doExecute(string $cmd, array $args): mixed
    {
        $sock = $this->ensureConnected();
        $id = $this->nextId++;
        $wireArgs = array_map(__NAMESPACE__ . '\\toWireValue', $args);

        // Encode request: [id, CMD, args]
        $body = MessagePack::pack([$id, strtoupper($cmd), $wireArgs]);
        $frame = pack('V', strlen($body)) . $body; // 4-byte LE u32 prefix

        if (fwrite($sock, $frame) === false) {
            $this->socket = null;
            throw SynapException::networkError('SynapRPC write failed');
        }

        // Read response frame header: 4-byte LE u32 length
        $lenBytes = $this->readExact($sock, 4);
        /** @var array{1: int} $unpacked */
        $unpacked = unpack('V', $lenBytes);
        $frameLen = $unpacked[1];
        $responseBody = $this->readExact($sock, $frameLen);

        $decoded = MessagePack::unpack($responseBody);
        // Response: [id, {Ok: WireValue} | {Err: string}]
        [, $resultEnv] = $decoded;

        if (isset($resultEnv['Ok'])) {
            return fromWireValue($resultEnv['Ok']);
        }

        $errMsg = is_string($resultEnv['Err'] ?? null)
            ? $resultEnv['Err']
            : 'unknown server error';
        throw SynapException::serverError($errMsg);
    }

    /**
     * Open a dedicated push connection, subscribe to topics, and block-loop
     * calling $onMessage for each server-push frame received.
     *
     * Push frames use id == 0xFFFFFFFF (U32_MAX) as the sentinel.
     * The loop exits when $shouldStop returns true or the connection closes.
     *
     * @param list<string>  $topics     Topic patterns to subscribe to
     * @param callable      $onMessage  Called with each decoded push message array
     * @param callable|null $shouldStop Optional predicate — loop exits when it returns true
     * @throws SynapException on connection or subscription failure
     */
    public function subscribePush(array $topics, callable $onMessage, ?callable $shouldStop = null): void
    {
        $errno  = 0;
        $errstr = '';
        /** @var resource|false $pushSock */
        $pushSock = @fsockopen($this->host, $this->port, $errno, $errstr, $this->timeoutSecs);
        if ($pushSock === false) {
            throw SynapException::networkError(
                "SynapRPC push connect to {$this->host}:{$this->port} failed ({$errno}): {$errstr}"
            );
        }
        stream_set_timeout($pushSock, $this->timeoutSecs);

        // Send SUBSCRIBE frame: [id=U32_MAX, "SUBSCRIBE", [topic, ...]]
        $PUSH_ID = 0xFFFF_FFFF;
        $wireTopics = array_map(__NAMESPACE__ . '\\toWireValue', $topics);
        $body  = MessagePack::pack([$PUSH_ID, 'SUBSCRIBE', $wireTopics]);
        $frame = pack('V', strlen($body)) . $body;

        if (fwrite($pushSock, $frame) === false) {
            fclose($pushSock);
            throw SynapException::networkError('SynapRPC push SUBSCRIBE write failed');
        }

        // Read and validate the initial SUBSCRIBE acknowledgement.
        $lenBytes = $this->readExactFrom($pushSock, 4);
        /** @var array{1: int} $unpacked */
        $unpacked = unpack('V', $lenBytes);
        $frameLen = $unpacked[1];
        $respBody = $this->readExactFrom($pushSock, $frameLen);
        $ack = MessagePack::unpack($respBody);
        if (isset($ack[1]['Err'])) {
            fclose($pushSock);
            throw SynapException::serverError((string) $ack[1]['Err']);
        }

        // Block-loop reading push frames.
        try {
            while (true) {
                if ($shouldStop !== null && ($shouldStop)()) {
                    break;
                }

                $lenBytes = @fread($pushSock, 4);
                if ($lenBytes === false || strlen($lenBytes) < 4) {
                    break; // Connection closed by server
                }

                /** @var array{1: int} $unpacked */
                $unpacked  = unpack('V', $lenBytes);
                $frameLen  = $unpacked[1];
                $pushBody  = $this->readExactFrom($pushSock, $frameLen);
                $decoded   = MessagePack::unpack($pushBody);
                [$frameId, $resultEnv] = $decoded;

                if ((int) $frameId !== $PUSH_ID) {
                    continue; // Skip non-push frames
                }

                $value = fromWireValue($resultEnv['Ok'] ?? null);
                if (is_array($value)) {
                    ($onMessage)($value);
                }
            }
        } finally {
            fclose($pushSock);
        }
    }

    /**
     * Read exactly $n bytes from the main socket, marking it closed on failure.
     *
     * @param resource $sock
     */
    private function readExact(mixed $sock, int $n): string
    {
        $buf = '';
        $remaining = $n;
        while ($remaining > 0) {
            $chunk = fread($sock, $remaining);
            if ($chunk === false || $chunk === '') {
                $this->socket = null;
                throw SynapException::networkError('SynapRPC connection closed unexpectedly');
            }
            $buf .= $chunk;
            $remaining -= strlen($chunk);
        }
        return $buf;
    }

    /**
     * Read exactly $n bytes from an arbitrary socket resource.
     *
     * @param resource $sock
     */
    private function readExactFrom(mixed $sock, int $n): string
    {
        $buf = '';
        $remaining = $n;
        while ($remaining > 0) {
            $chunk = fread($sock, $remaining);
            if ($chunk === false || $chunk === '') {
                throw SynapException::networkError('SynapRPC connection closed unexpectedly');
            }
            $buf .= $chunk;
            $remaining -= strlen($chunk);
        }
        return $buf;
    }

    /**
     * Close the persistent socket connection.
     */
    public function close(): void
    {
        if ($this->socket !== null) {
            fclose($this->socket);
            $this->socket = null;
        }
    }
}

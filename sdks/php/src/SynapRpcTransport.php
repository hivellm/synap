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
    /**
     * The reserved id that marks a frame as server push.
     *
     * It identifies frames coming *from* the server. A client must never send
     * it as a request id -- the server refuses such a frame outright.
     */
    private const PUSH_ID = 0xFFFF_FFFF;

    /** @var resource|null */
    private mixed $socket = null;

    private int $nextId = 1;

    /**
     * @param string      $host        SynapRPC host.
     * @param int         $port        SynapRPC port.
     * @param int         $timeoutSecs Connect and per-call timeout.
     * @param string|null $authToken   API key / token, sent as `AUTH <token>`.
     * @param string|null $username    Username, sent with $password as `AUTH <user> <pass>`.
     * @param string|null $password    Password paired with $username.
     */
    public function __construct(
        private readonly string $host,
        private readonly int $port,
        private readonly int $timeoutSecs,
        private readonly ?string $authToken = null,
        private readonly ?string $username = null,
        private readonly ?string $password = null,
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

        $this->authenticate($sock);

        return $sock;
    }

    /**
     * Send AUTH on a freshly opened socket.
     *
     * This transport never authenticated, so it could not reach a
     * `require_auth` deployment on 15501 at all — every command came back
     * NOAUTH. `AUTH <password>` authenticates the default user and
     * `AUTH <user> <pass>` names one, matching the server's handshake. With no
     * credentials configured the connection stays anonymous, which is what an
     * open deployment expects.
     *
     * @param resource $sock
     * @throws SynapException when the credentials are rejected
     */
    private function authenticate(mixed $sock): void
    {
        try {
            $this->authenticateOn($sock);
        } catch (SynapException $e) {
            // Do not leave an unauthenticated socket behind for the retry path
            // to reuse: it would fail every command with NOAUTH instead.
            fclose($sock);
            $this->socket = null;
            throw $e;
        }
    }

    /**
     * Write an AUTH frame on the given socket and consume its acknowledgement.
     *
     * Framing is done here rather than through {@see doExecute} so the
     * dedicated push socket, which is not `$this->socket`, can authenticate
     * with the same code.
     *
     * @param resource $sock
     * @throws SynapException when the credentials are rejected
     */
    private function authenticateOn(mixed $sock): void
    {
        if ($this->username !== null && $this->username !== '' && $this->password !== null) {
            $args = [$this->username, $this->password];
        } elseif ($this->authToken !== null && $this->authToken !== '') {
            $args = [$this->authToken];
        } else {
            return;
        }

        $wireArgs = array_map(__NAMESPACE__ . '\\toWireValue', $args);
        $body  = MessagePack::pack([0, 'AUTH', $wireArgs]);
        $frame = pack('V', strlen($body)) . $body;

        if (fwrite($sock, $frame) === false) {
            throw SynapException::networkError('SynapRPC AUTH write failed');
        }

        /** @var array{1: int} $unpacked */
        $unpacked = unpack('V', $this->readExactFrom($sock, 4));
        $ack = MessagePack::unpack($this->readExactFrom($sock, $unpacked[1]));
        if (isset($ack[1]['Err'])) {
            throw SynapException::serverError((string) $ack[1]['Err']);
        }
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

        // A push connection authenticates like any other: SUBSCRIBE is a
        // privileged command, so credentials have to open this socket too.
        $this->authenticateOn($pushSock);

        // Send SUBSCRIBE frame: [id, "SUBSCRIBE", [topic, ...]]
        //
        // The id is an ordinary request id. This used to send 0xFFFFFFFF — the
        // reserved push sentinel — as the *request* id, which the server
        // refuses outright ("request id u32::MAX is reserved for server push
        // frames"), so pub/sub over SynapRPC could not work at all. The
        // sentinel identifies frames coming *from* the server; it is not an
        // address a client may send to.
        $wireTopics = array_map(__NAMESPACE__ . '\\toWireValue', $topics);
        $body  = MessagePack::pack([1, 'SUBSCRIBE', $wireTopics]);
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

                if ((int) $frameId !== self::PUSH_ID) {
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

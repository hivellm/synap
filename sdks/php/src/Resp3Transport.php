<?php

declare(strict_types=1);

namespace Synap\SDK;

use Synap\SDK\Exception\SynapException;

/**
 * Blocking TCP transport using the RESP3 (Redis-compatible) text protocol.
 *
 * On first connection the transport sends HELLO 3 to negotiate RESP3.
 * Commands are sent as RESP2 multibulk inline frames for broad compatibility:
 *
 *   *N\r\n
 *   $<len>\r\n<arg>\r\n  × N
 *
 * Responses are parsed according to the RESP3 type prefix:
 *   +  simple string
 *   -  error
 *   :  integer
 *   ,  double
 *   #  boolean
 *   _  null
 *   $  bulk string
 *   *  array
 *   %  map
 *   ~  set (returned as array)
 */
class Resp3Transport
{
    /** @var resource|null */
    private mixed $socket = null;

    public function __construct(
        private readonly string $host,
        private readonly int $port,
        private readonly int $timeoutSecs,
    ) {}

    /**
     * Ensure a RESP3 connection exists, (re)connecting and negotiating on demand.
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
                "RESP3 connect to {$this->host}:{$this->port} failed ({$errno}): {$errstr}"
            );
        }
        stream_set_timeout($sock, $this->timeoutSecs);
        $this->socket = $sock;

        // Negotiate RESP3 protocol
        $hello = "*2\r\n\$5\r\nHELLO\r\n\$1\r\n3\r\n";
        if (fwrite($sock, $hello) === false) {
            fclose($sock);
            $this->socket = null;
            throw SynapException::networkError('RESP3 HELLO write failed');
        }
        // Drain the HELLO response (map of server info)
        $this->readValue($sock);

        return $sock;
    }

    /**
     * Execute a command and return the parsed RESP3 response.
     *
     * Automatically reconnects once on network failure.
     *
     * @param string      $cmd  Command name (e.g. "GET", "SET")
     * @param list<mixed> $args Command arguments — serialised as bulk strings
     * @return mixed Decoded PHP value
     * @throws SynapException on network or server error
     */
    public function execute(string $cmd, array $args): mixed
    {
        try {
            return $this->doExecute($cmd, $args);
        } catch (SynapException $e) {
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
        $parts = array_merge([strtoupper($cmd)], $args);
        $frame = '*' . count($parts) . "\r\n";
        foreach ($parts as $part) {
            $encoded = (string) $part;
            $frame .= '$' . strlen($encoded) . "\r\n" . $encoded . "\r\n";
        }
        if (fwrite($sock, $frame) === false) {
            $this->socket = null;
            throw SynapException::networkError('RESP3 write failed');
        }
        return $this->readValue($sock);
    }

    /**
     * Parse one RESP3 value from the socket.
     *
     * @param resource $sock
     * @return mixed
     */
    private function readValue(mixed $sock): mixed
    {
        $line = fgets($sock);
        if ($line === false) {
            $this->socket = null;
            throw SynapException::networkError('RESP3 connection closed');
        }
        $line   = rtrim($line, "\r\n");
        $prefix = $line[0];
        $rest   = substr($line, 1);

        switch ($prefix) {
            case '+':
                return $rest;
            case '-':
                throw SynapException::serverError($rest);
            case ':':
                return (int) $rest;
            case ',':
                return (float) $rest;
            case '#':
                return strtolower($rest) === 't';
            case '_':
                return null;
            case '$': {
                $len = (int) $rest;
                if ($len === -1) {
                    return null;
                }
                // Read $len bytes + CRLF
                $data = $this->readExact($sock, $len + 2);
                return substr($data, 0, $len);
            }
            case '*': {
                $count = (int) $rest;
                if ($count === -1) {
                    return null;
                }
                $arr = [];
                for ($i = 0; $i < $count; $i++) {
                    $arr[] = $this->readValue($sock);
                }
                return $arr;
            }
            case '%': {
                // RESP3 map — returned as associative array
                $count = (int) $rest;
                $map = [];
                for ($i = 0; $i < $count; $i++) {
                    $k       = (string) $this->readValue($sock);
                    $map[$k] = $this->readValue($sock);
                }
                return $map;
            }
            case '~': {
                // RESP3 set type — returned as plain list
                $count = (int) $rest;
                $set = [];
                for ($i = 0; $i < $count; $i++) {
                    $set[] = $this->readValue($sock);
                }
                return $set;
            }
            default:
                return $rest;
        }
    }

    /**
     * Read exactly $n bytes from the socket.
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
                throw SynapException::networkError('RESP3 connection closed unexpectedly');
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

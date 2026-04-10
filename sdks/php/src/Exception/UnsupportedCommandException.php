<?php

declare(strict_types=1);

namespace Synap\SDK\Exception;

use Exception;

/**
 * Raised when a command has no native mapping for the active transport.
 *
 * Native transports (SynapRPC, RESP3) can only execute commands that are
 * mapped to wire commands in Transport.php.  Unmapped commands silently fell
 * back to HTTP in older SDK versions.  That fallback has been removed — callers
 * must use an HTTP transport, or switch to a mapped command.
 */
class UnsupportedCommandException extends Exception
{
    public function __construct(
        public readonly string $command,
        public readonly string $transport,
    ) {
        parent::__construct(
            "command '{$command}' is not supported on transport '{$transport}'"
        );
    }
}

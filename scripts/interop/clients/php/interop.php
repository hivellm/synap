<?php

declare(strict_types=1);

/**
 * Interop cell: PHP SDK against a Thunder-based server.
 *
 * Driven by scripts/interop/run-matrix.py. Prints one
 * `STEP <name> PASS|FAIL <detail>` line per step and exits non-zero if any
 * step failed.
 *
 * PHP has no Thunder package, so the SDK keeps its hand-written transport.
 * This cell proves that transport still agrees with the Thunder server.
 */

require __DIR__ . '/../../../../sdks/php/vendor/autoload.php';

use Synap\SDK\SynapConfig;
use Synap\SDK\SynapClient;
use Synap\SDK\Exception\SynapException;

// Not valid UTF-8, so a transport that quietly round-trips through a string
// cannot pass the binary step.
$binary = "\xDE\xAD\xBE\xEF";
$topic  = 'interop.php';

$failures = 0;

function report(string $step, bool $ok, string $detail): void
{
    global $failures;
    echo "STEP {$step} " . ($ok ? 'PASS' : 'FAIL') . " {$detail}\n";
    if (!$ok) {
        $failures++;
    }
}

[$script, $host, $port, $user, $pass] = $argv;

$config = (new SynapConfig("synap://{$host}:{$port}"))
    ->withBasicAuth($user, $pass)
    ->withTimeout(15);
$client = new SynapClient($config);
$rpc = $client->getSynapRpcTransport();

if ($rpc === null) {
    report('auth', false, 'synap:// URL did not select the SynapRPC transport');
    exit(1);
}

// 1. Authenticate.
//
//    EXISTS rather than PING: the server answers PING before authentication,
//    so a PING probe passes just as happily on a connection that never
//    authenticated -- exactly the bug this column exists to catch.
try {
    $probe = $rpc->execute('EXISTS', ['interop:php:probe']);
    report('auth', true, 'EXISTS -> ' . var_export($probe, true));
} catch (SynapException $e) {
    report('auth', false, get_class($e) . ': ' . $e->getMessage());
    exit(1);
}

// 2. SET/GET a binary value -- canonical MessagePack bin, byte-exact back.
try {
    $rpc->execute('SET', ['interop:php:bin', $binary]);
    $got = $rpc->execute('GET', ['interop:php:bin']);
    $got = is_string($got) ? $got : '';
    report('kv_binary', $got === $binary, bin2hex($binary) . ' -> ' . bin2hex($got));
} catch (SynapException $e) {
    report('kv_binary', false, get_class($e) . ': ' . $e->getMessage());
}

// 3. SUBSCRIBE then PUBLISH.
//
//    subscribePush blocks on its own socket, so the publish has to come from
//    somewhere else: a forked child would need pcntl, which is not available
//    on Windows. Instead the subscriber publishes to itself over the main
//    connection from inside the stop predicate, which runs between reads.
try {
    $received = [];
    $published = false;

    $rpc->subscribePush(
        [$topic],
        function (array $msg) use (&$received): void {
            $received[] = $msg;
        },
        function () use (&$received, &$published, $rpc, $topic): bool {
            if (!$published) {
                $published = true;
                $rpc->execute('PUBLISH', [$topic, 'interop-payload']);
                return false;
            }
            return count($received) > 0;
        }
    );

    $ok = count($received) > 0 && ($received[0]['topic'] ?? null) === $topic;
    report('pubsub', $ok, 'received=' . json_encode(array_slice($received, 0, 1)));
} catch (SynapException $e) {
    report('pubsub', false, get_class($e) . ': ' . $e->getMessage());
}

// 4. Error round-trip -- an unknown command must throw, and must not poison
//    the connection.
try {
    $result = $rpc->execute('NOSUCHCOMMAND', []);
    report('error', false, 'expected an exception, got ' . var_export($result, true));
} catch (SynapException $e) {
    $alive = true;
    try {
        $rpc->execute('EXISTS', ['interop:php:probe']);
    } catch (SynapException) {
        $alive = false;
    }
    report('error', $alive, 'threw ' . get_class($e) . '; connection alive=' . var_export($alive, true));
}

$rpc->close();
exit($failures > 0 ? 1 : 0);

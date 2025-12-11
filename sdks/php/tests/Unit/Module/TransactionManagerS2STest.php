<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\TransactionManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

/**
 * @group s2s
 * @requires extension curl
 */
final class TransactionManagerS2STest extends TestCase
{
    private TransactionManager $transaction;
    private SynapClient $client;

    protected function setUp(): void
    {
        $url = getenv('SYNAP_URL') ?: 'http://localhost:15500';
        $config = new SynapConfig($url);
        $this->client = new SynapClient($config);
        $this->transaction = $this->client->transaction();
    }

    public function testMultiExec(): void
    {
        $clientId = 'test:' . uniqid();

        // Start transaction
        $result = $this->transaction->multi($clientId);
        $this->assertTrue($result['success']);

        // Queue commands using execute with client_id (automatic queuing)
        $this->client->execute('kv.set', 'tx:key1', ['value' => 'value1', 'client_id' => $clientId]);
        $this->client->execute('kv.set', 'tx:key2', ['value' => 'value2', 'client_id' => $clientId]);

        // Execute transaction
        $execResult = $this->transaction->exec($clientId);
        $this->assertTrue($execResult['success']);
        $this->assertArrayHasKey('results', $execResult);
        $this->assertCount(2, $execResult['results']);

        // Verify values were set
        $value1 = $this->client->kv()->get('tx:key1');
        $value2 = $this->client->kv()->get('tx:key2');
        $this->assertEquals('value1', $value1);
        $this->assertEquals('value2', $value2);
    }

    public function testDiscard(): void
    {
        $clientId = 'test:' . uniqid();

        // Start transaction
        $this->transaction->multi($clientId);

        // Queue a command (will be discarded)
        $this->client->execute('kv.set', 'tx:discard:key', ['value' => 'value', 'client_id' => $clientId]);

        // Discard transaction
        $result = $this->transaction->discard($clientId);
        $this->assertTrue($result['success']);

        // Verify value was NOT set
        $value = $this->client->kv()->get('tx:discard:key');
        $this->assertNull($value);
    }

    public function testWatchUnwatch(): void
    {
        $clientId = 'test:' . uniqid();

        // Start transaction
        $this->transaction->multi($clientId);

        // Watch keys
        $result = $this->transaction->watch(['watch:key1', 'watch:key2'], $clientId);
        $this->assertTrue($result['success']);

        // Unwatch
        $result = $this->transaction->unwatch($clientId);
        $this->assertTrue($result['success']);
    }

    public function testWatchAbortOnConflict(): void
    {
        $clientId = 'test:' . uniqid();

        // Set initial value
        $this->client->kv()->set('watch:conflict:key', 'initial');

        // Start transaction and watch
        $this->transaction->multi($clientId);
        $this->transaction->watch(['watch:conflict:key'], $clientId);

        // Modify watched key from another client (simulate conflict)
        $this->client->kv()->set('watch:conflict:key', 'modified');

        // Try to execute transaction (should abort)
        $execResult = $this->transaction->exec($clientId);
        $this->assertFalse($execResult['success']);
        $this->assertTrue($execResult['aborted'] ?? false);
    }

    public function testEmptyTransaction(): void
    {
        $clientId = 'test:' . uniqid();

        // Start transaction
        $this->transaction->multi($clientId);

        // Execute without queuing commands
        $execResult = $this->transaction->exec($clientId);
        $this->assertTrue($execResult['success']);
        $this->assertEmpty($execResult['results'] ?? []);
    }
}


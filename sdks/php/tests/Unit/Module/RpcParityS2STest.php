<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Exception\UnsupportedCommandException;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

/**
 * RPC-parity S2S tests — queues, streams, pub/sub, transactions, scripts.
 *
 * Tests run across all three transports: HTTP, SynapRPC (synap://), RESP3 (resp3://).
 *
 * Enable:  SYNAP_S2S=true phpunit --group s2s
 * Env vars (optional overrides):
 *   SYNAP_HTTP_URL   (default: http://localhost:15500)
 *   SYNAP_RPC_URL    (default: synap://localhost:15501)
 *   SYNAP_RESP3_URL  (default: resp3://localhost:6379)
 *
 * @group s2s
 */
class RpcParityS2STest extends TestCase
{
    protected function setUp(): void
    {
        parent::setUp();

        if (getenv('SYNAP_S2S') !== 'true') {
            $this->markTestSkipped('S2S tests disabled (set SYNAP_S2S=true to enable)');
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private function httpClient(): SynapClient
    {
        $url = getenv('SYNAP_HTTP_URL') ?: 'http://localhost:15500';
        return new SynapClient(new SynapConfig($url));
    }

    private function rpcClient(): SynapClient
    {
        $url = getenv('SYNAP_RPC_URL') ?: 'synap://localhost:15501';
        return new SynapClient(new SynapConfig($url));
    }

    private function resp3Client(): SynapClient
    {
        $url = getenv('SYNAP_RESP3_URL') ?: 'resp3://localhost:6379';
        return new SynapClient(new SynapConfig($url));
    }

    private function uid(): string
    {
        return substr(str_replace('-', '', (string) \Ramsey\Uuid\Uuid::uuid4()), 0, 8);
    }

    // ── Queue tests ───────────────────────────────────────────────────────────

    public function testQueueRoundTripHttp(): void
    {
        $this->queueRoundTrip($this->httpClient());
    }

    public function testQueueRoundTripRpc(): void
    {
        $this->queueRoundTrip($this->rpcClient());
    }

    public function testQueueRoundTripResp3(): void
    {
        $this->queueRoundTrip($this->resp3Client());
    }

    private function queueRoundTrip(SynapClient $client): void
    {
        $name = 'test-q-' . bin2hex(random_bytes(4));
        $client->queue()->createQueue($name, 100, 60);

        $msgId = $client->queue()->publish($name, ['data' => 'hello'], 5);
        $this->assertIsString($msgId);
        $this->assertNotEmpty($msgId);

        $msg = $client->queue()->consume($name, 'worker-1');
        $this->assertNotNull($msg);
        $this->assertEquals(['data' => 'hello'], $msg->payload);
        $this->assertEquals(5, $msg->priority);

        $client->queue()->ack($name, $msg->id);
    }

    public function testQueueEmptyReturnsNullHttp(): void
    {
        $this->queueEmpty($this->httpClient());
    }

    public function testQueueEmptyReturnsNullRpc(): void
    {
        $this->queueEmpty($this->rpcClient());
    }

    private function queueEmpty(SynapClient $client): void
    {
        $name = 'test-q-empty-' . bin2hex(random_bytes(4));
        $client->queue()->createQueue($name);
        $msg = $client->queue()->consume($name, 'worker-1');
        $this->assertNull($msg);
    }

    public function testQueueListHttp(): void
    {
        $this->queueList($this->httpClient());
    }

    public function testQueueListRpc(): void
    {
        $this->queueList($this->rpcClient());
    }

    private function queueList(SynapClient $client): void
    {
        $name = 'test-q-list-' . bin2hex(random_bytes(4));
        $client->queue()->createQueue($name);
        $queues = $client->queue()->list();
        $this->assertContains($name, $queues);
    }

    // ── Stream tests ──────────────────────────────────────────────────────────

    public function testStreamRoundTripHttp(): void
    {
        $this->streamRoundTrip($this->httpClient());
    }

    public function testStreamRoundTripRpc(): void
    {
        $this->streamRoundTrip($this->rpcClient());
    }

    public function testStreamRoundTripResp3(): void
    {
        $this->streamRoundTrip($this->resp3Client());
    }

    private function streamRoundTrip(SynapClient $client): void
    {
        $room = 'test-room-' . bin2hex(random_bytes(4));
        $client->stream()->createRoom($room);

        $off0 = $client->stream()->publish($room, 'user.created', ['userId' => 'u1']);
        $off1 = $client->stream()->publish($room, 'user.updated', ['userId' => 'u1', 'name' => 'Alice']);
        $this->assertIsInt($off0);
        $this->assertIsInt($off1);
        $this->assertGreaterThan($off0, $off1);

        $events = $client->stream()->read($room, 0);
        $this->assertGreaterThanOrEqual(2, count($events));
        $this->assertEquals('user.created', $events[0]->event);
        $this->assertEquals('user.updated', $events[1]->event);
    }

    public function testStreamListRoomsHttp(): void
    {
        $this->streamListRooms($this->httpClient());
    }

    public function testStreamListRoomsRpc(): void
    {
        $this->streamListRooms($this->rpcClient());
    }

    private function streamListRooms(SynapClient $client): void
    {
        $room = 'test-room-list-' . bin2hex(random_bytes(4));
        $client->stream()->createRoom($room);
        $rooms = $client->stream()->listRooms();
        $this->assertContains($room, $rooms);
    }

    // ── Pub/Sub tests ─────────────────────────────────────────────────────────

    public function testPubSubPublishHttp(): void
    {
        $this->pubSubPublish($this->httpClient());
    }

    public function testPubSubPublishRpc(): void
    {
        $this->pubSubPublish($this->rpcClient());
    }

    public function testPubSubPublishResp3(): void
    {
        $this->pubSubPublish($this->resp3Client());
    }

    private function pubSubPublish(SynapClient $client): void
    {
        $topic  = 'test.pub.' . bin2hex(random_bytes(4));
        $result = $client->pubsub()->publish($topic, ['msg' => 'hello']);
        $this->assertIsInt($result);
        $this->assertGreaterThanOrEqual(0, $result);
    }

    // ── Transaction tests ─────────────────────────────────────────────────────

    public function testTransactionMultiExecHttp(): void
    {
        $this->transactionRoundTrip($this->httpClient());
    }

    public function testTransactionMultiExecRpc(): void
    {
        $this->transactionRoundTrip($this->rpcClient());
    }

    private function transactionRoundTrip(SynapClient $client): void
    {
        $clientId = 'txn-' . bin2hex(random_bytes(4));
        $key      = 'tx:test:' . bin2hex(random_bytes(4));

        $client->sendCommand('transaction.multi', ['client_id' => $clientId]);
        $client->sendCommand('kv.set', ['key' => $key, 'value' => 'txn-value', 'client_id' => $clientId]);
        $result = $client->sendCommand('transaction.exec', ['client_id' => $clientId]);

        $this->assertTrue($result['success'] ?? false);
        $value = $client->kv()->get($key);
        $this->assertEquals('txn-value', $value);
    }

    // ── Script tests ──────────────────────────────────────────────────────────

    public function testScriptEvalHttp(): void
    {
        $this->scriptEval($this->httpClient());
    }

    public function testScriptEvalRpc(): void
    {
        $this->scriptEval($this->rpcClient());
    }

    private function scriptEval(SynapClient $client): void
    {
        $response = $client->sendCommand('script.eval', [
            'script' => 'return 42',
            'keys'   => [],
            'args'   => [],
        ]);
        $this->assertNotNull($response);
    }

    // ── UnsupportedCommandException regression ────────────────────────────────

    public function testRpcRaisesUnsupportedCommandForBitmap(): void
    {
        $client = $this->rpcClient();
        $this->expectException(UnsupportedCommandException::class);
        $client->sendCommand('bitmap.setbit', ['key' => 'bm', 'offset' => 7, 'value' => 1]);
    }

    public function testResp3RaisesUnsupportedCommandForBitmap(): void
    {
        $client = $this->resp3Client();
        $this->expectException(UnsupportedCommandException::class);
        $client->sendCommand('bitmap.setbit', ['key' => 'bm', 'offset' => 7, 'value' => 1]);
    }

    public function testHttpDoesNotRaiseUnsupportedCommand(): void
    {
        $client = $this->httpClient();
        try {
            $client->sendCommand('bitmap.setbit', ['key' => 'bm:' . bin2hex(random_bytes(4)), 'offset' => 7, 'value' => 1]);
            // Success is also fine
            $this->assertTrue(true);
        } catch (UnsupportedCommandException $e) {
            $this->fail('HTTP transport must not raise UnsupportedCommandException');
        } catch (\Throwable) {
            // Server error is OK — the command was routed
            $this->assertTrue(true);
        }
    }
}

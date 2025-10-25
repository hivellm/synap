<?php

declare(strict_types=1);

namespace Synap\SDK;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\GuzzleException;
use GuzzleHttp\RequestOptions;
use Synap\SDK\Exception\SynapException;
use Synap\SDK\Module\KVStore;
use Synap\SDK\Module\PubSubManager;
use Synap\SDK\Module\QueueManager;
use Synap\SDK\Module\StreamManager;

/**
 * Main Synap SDK client
 */
class SynapClient
{
    private Client $httpClient;
    private SynapConfig $config;
    private ?KVStore $kv = null;
    private ?QueueManager $queue = null;
    private ?StreamManager $stream = null;
    private ?PubSubManager $pubsub = null;

    public function __construct(SynapConfig $config)
    {
        $this->config = $config;
        $this->httpClient = new Client([
            'base_uri' => $config->getBaseUrl(),
            'timeout' => $config->getTimeout(),
            'headers' => $this->buildHeaders(),
        ]);
    }

    public function kv(): KVStore
    {
        if ($this->kv === null) {
            $this->kv = new KVStore($this);
        }

        return $this->kv;
    }

    public function queue(): QueueManager
    {
        if ($this->queue === null) {
            $this->queue = new QueueManager($this);
        }

        return $this->queue;
    }

    public function stream(): StreamManager
    {
        if ($this->stream === null) {
            $this->stream = new StreamManager($this);
        }

        return $this->stream;
    }

    public function pubsub(): PubSubManager
    {
        if ($this->pubsub === null) {
            $this->pubsub = new PubSubManager($this);
        }

        return $this->pubsub;
    }

    /**
     * Execute StreamableHTTP operation
     *
     * @param string $operation Operation type (e.g., 'kv.set', 'queue.publish')
     * @param string $target Target resource (e.g., key name, queue name)
     * @param array<string, mixed> $data Operation data
     * @return array<string, mixed>
     */
    public function execute(string $operation, string $target, array $data = []): array
    {
        try {
            $payload = [
                'operation' => $operation,
                'target' => $target,
                'data' => $data,
            ];

            $options = [
                RequestOptions::JSON => $payload,
                RequestOptions::HEADERS => $this->buildHeaders(),
            ];

            $response = $this->httpClient->request('POST', '/api/stream', $options);
            $body = (string) $response->getBody();

            if (empty($body)) {
                return [];
            }

            $result = json_decode($body, true);

            if (json_last_error() !== JSON_ERROR_NONE) {
                throw SynapException::invalidResponse('Failed to parse JSON response');
            }

            if (! is_array($result)) {
                return [];
            }

            // Check for server error in response
            if (isset($result['error'])) {
                $error = $result['error'];
                $errorMessage = is_string($error) ? $error : json_encode($error);

                assert(is_string($errorMessage));

                throw SynapException::serverError($errorMessage);
            }

            /** @var array<string, mixed> $result */
            return $result;
        } catch (GuzzleException $e) {
            throw SynapException::networkError($e->getMessage());
        }
    }

    /**
     * @return array<string, string>
     */
    private function buildHeaders(): array
    {
        $headers = [
            'Content-Type' => 'application/json',
            'Accept' => 'application/json',
        ];

        if ($this->config->getAuthToken() !== null) {
            $headers['Authorization'] = 'Bearer ' . $this->config->getAuthToken();
        }

        return $headers;
    }

    public function getConfig(): SynapConfig
    {
        return $this->config;
    }
}

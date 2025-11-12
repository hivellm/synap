<?php

declare(strict_types=1);

namespace Synap\SDK;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\GuzzleException;
use GuzzleHttp\RequestOptions;
use Synap\SDK\Exception\SynapException;
use Synap\SDK\Module\BitmapManager;
use Synap\SDK\Module\GeospatialManager;
use Synap\SDK\Module\HashManager;
use Synap\SDK\Module\HyperLogLogManager;
use Synap\SDK\Module\KVStore;
use Synap\SDK\Module\ListManager;
use Synap\SDK\Module\PubSubManager;
use Synap\SDK\Module\QueueManager;
use Synap\SDK\Module\SetManager;
use Synap\SDK\Module\StreamManager;
use Synap\SDK\Module\TransactionManager;

/**
 * Main Synap SDK client
 */
class SynapClient
{
    private Client $httpClient;
    private SynapConfig $config;
    private ?KVStore $kv = null;
    private ?HashManager $hash = null;
    private ?ListManager $list = null;
    private ?SetManager $set = null;
    private ?QueueManager $queue = null;
    private ?StreamManager $stream = null;
    private ?PubSubManager $pubsub = null;
    private ?BitmapManager $bitmap = null;
    private ?HyperLogLogManager $hyperloglog = null;
    private ?GeospatialManager $geospatial = null;
    private ?TransactionManager $transaction = null;

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

    public function hash(): HashManager
    {
        if ($this->hash === null) {
            $this->hash = new HashManager($this);
        }

        return $this->hash;
    }

    public function list(): ListManager
    {
        if ($this->list === null) {
            $this->list = new ListManager($this);
        }

        return $this->list;
    }

    public function set(): SetManager
    {
        if ($this->set === null) {
            $this->set = new SetManager($this);
        }

        return $this->set;
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

    public function bitmap(): BitmapManager
    {
        if ($this->bitmap === null) {
            $this->bitmap = new BitmapManager($this);
        }

        return $this->bitmap;
    }

    public function hyperloglog(): HyperLogLogManager
    {
        if ($this->hyperloglog === null) {
            $this->hyperloglog = new HyperLogLogManager($this);
        }

        return $this->hyperloglog;
    }

    public function geospatial(): GeospatialManager
    {
        if ($this->geospatial === null) {
            $this->geospatial = new GeospatialManager($this);
        }

        return $this->geospatial;
    }

    public function transaction(): TransactionManager
    {
        if ($this->transaction === null) {
            $this->transaction = new TransactionManager($this);
        }

        return $this->transaction;
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
            // Generate request ID (UUID v4)
            $requestId = $this->generateUuid();

            // Include target in data if it's not empty (for commands that use 'key' or 'destination')
            $payloadData = $data;
            if (! empty($target)) {
                // Determine field name based on operation
                if (str_starts_with($operation, 'bitmap.bitop') || str_starts_with($operation, 'hyperloglog.pfmerge')) {
                    $payloadData['destination'] = $target;
                } else {
                    $payloadData['key'] = $target;
                }
            }

            // StreamableHTTP format: {command, payload, request_id}
            $payload = [
                'command' => $operation,
                'payload' => $payloadData,
                'request_id' => $requestId,
            ];

            $options = [
                RequestOptions::JSON => $payload,
                RequestOptions::HEADERS => $this->buildHeaders(),
            ];

            $response = $this->httpClient->request('POST', '/api/v1/command', $options);
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

            // Check for server error in StreamableHTTP response
            if (isset($result['success']) && $result['success'] === false) {
                $error = $result['error'] ?? 'Unknown error';
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
     * Generate UUID v4
     *
     * @return string
     */
    private function generateUuid(): string
    {
        $data = random_bytes(16);
        $data[6] = chr(ord($data[6]) & 0x0f | 0x40); // Version 4
        $data[8] = chr(ord($data[8]) & 0x3f | 0x80); // Variant 10

        return sprintf(
            '%08s-%04s-%04s-%04s-%12s',
            bin2hex(substr($data, 0, 4)),
            bin2hex(substr($data, 4, 2)),
            bin2hex(substr($data, 6, 2)),
            bin2hex(substr($data, 8, 2)),
            bin2hex(substr($data, 10, 6))
        );
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

        // Add authentication headers
        if ($this->config->getAuthToken() !== null) {
            $headers['Authorization'] = 'Bearer ' . $this->config->getAuthToken();
        } elseif ($this->config->getUsername() !== null && $this->config->getPassword() !== null) {
            $credentials = base64_encode(
                $this->config->getUsername() . ':' . $this->config->getPassword()
            );
            $headers['Authorization'] = 'Basic ' . $credentials;
        }

        return $headers;
    }

    public function getConfig(): SynapConfig
    {
        return $this->config;
    }
}

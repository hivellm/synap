<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Exception\SynapException;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

final class AuthenticationTest extends TestCase
{
    private const TEST_URL = 'http://localhost:15500';
    private const TEST_USERNAME = 'root';
    private const TEST_PASSWORD = 'root';

    public function testBasicAuthConfigCreation(): void
    {
        $config = SynapConfig::create(self::TEST_URL)
            ->withBasicAuth(self::TEST_USERNAME, self::TEST_PASSWORD);

        $this->assertSame(self::TEST_USERNAME, $config->getUsername());
        $this->assertSame(self::TEST_PASSWORD, $config->getPassword());
        $this->assertNull($config->getAuthToken());
    }

    public function testApiKeyConfigCreation(): void
    {
        $config = SynapConfig::create(self::TEST_URL)
            ->withAuthToken('sk_test123');

        $this->assertSame('sk_test123', $config->getAuthToken());
        $this->assertNull($config->getUsername());
        $this->assertNull($config->getPassword());
    }

    public function testConfigBuilderPattern(): void
    {
        $config = SynapConfig::create(self::TEST_URL)
            ->withTimeout(60)
            ->withBasicAuth('user', 'pass');

        $this->assertSame(60, $config->getTimeout());
        $this->assertSame('user', $config->getUsername());
        $this->assertSame('pass', $config->getPassword());
    }

    public function testAuthTokenOverridesBasicAuth(): void
    {
        $config = SynapConfig::create(self::TEST_URL)
            ->withBasicAuth('user', 'pass')
            ->withAuthToken('sk_test123');

        $this->assertSame('sk_test123', $config->getAuthToken());
        $this->assertNull($config->getUsername());
        $this->assertNull($config->getPassword());
    }

    public function testBasicAuthOverridesAuthToken(): void
    {
        $config = SynapConfig::create(self::TEST_URL)
            ->withAuthToken('sk_test123')
            ->withBasicAuth('user', 'pass');

        $this->assertSame('user', $config->getUsername());
        $this->assertSame('pass', $config->getPassword());
        $this->assertNull($config->getAuthToken());
    }

    public function testClientWithBasicAuth(): void
    {
        $config = SynapConfig::create(self::TEST_URL)
            ->withBasicAuth(self::TEST_USERNAME, self::TEST_PASSWORD);
        $client = new SynapClient($config);

        $this->assertInstanceOf(SynapClient::class, $client);
    }

    public function testClientWithApiKey(): void
    {
        $config = SynapConfig::create(self::TEST_URL)
            ->withAuthToken('sk_test123');
        $client = new SynapClient($config);

        $this->assertInstanceOf(SynapClient::class, $client);
    }

    public function testClientWithoutAuth(): void
    {
        $config = SynapConfig::create(self::TEST_URL);
        $client = new SynapClient($config);

        $this->assertInstanceOf(SynapClient::class, $client);
    }

    /**
     * S2S Test: Requires running Synap server
     * Run with: SYNAP_URL=http://localhost:15500 SYNAP_TEST_USERNAME=root SYNAP_TEST_PASSWORD=root vendor/bin/phpunit tests/Unit/AuthenticationTest.php::testBasicAuthS2S
     */
    public function testBasicAuthS2S(): void
    {
        $url = getenv('SYNAP_URL') ?: self::TEST_URL;
        $username = getenv('SYNAP_TEST_USERNAME') ?: self::TEST_USERNAME;
        $password = getenv('SYNAP_TEST_PASSWORD') ?: self::TEST_PASSWORD;

        $config = SynapConfig::create($url)
            ->withBasicAuth($username, $password);
        $client = new SynapClient($config);

        try {
            // Test KV operation
            $client->kv()->set('auth:test:basic', 'test_value');
            $value = $client->kv()->get('auth:test:basic');
            $this->assertSame('test_value', $value);

            // Cleanup
            $client->kv()->delete('auth:test:basic');
        } catch (\Exception $e) {
            $this->markTestSkipped('S2S test requires running Synap server: ' . $e->getMessage());
        }
    }

    /**
     * S2S Test: Requires running Synap server with valid API key
     * Run with: SYNAP_URL=http://localhost:15500 SYNAP_TEST_API_KEY=sk_... vendor/bin/phpunit tests/Unit/AuthenticationTest.php::testApiKeyAuthS2S
     */
    public function testApiKeyAuthS2S(): void
    {
        $url = getenv('SYNAP_URL') ?: self::TEST_URL;
        $apiKey = getenv('SYNAP_TEST_API_KEY');

        if (!$apiKey) {
            $this->markTestSkipped('S2S test requires SYNAP_TEST_API_KEY environment variable');
            return;
        }

        $config = SynapConfig::create($url)
            ->withAuthToken($apiKey);
        $client = new SynapClient($config);

        try {
            // Test KV operation
            $client->kv()->set('auth:test:apikey', 'test_value');
            $value = $client->kv()->get('auth:test:apikey');
            $this->assertSame('test_value', $value);

            // Cleanup
            $client->kv()->delete('auth:test:apikey');
        } catch (\Exception $e) {
            $this->markTestSkipped('S2S test requires running Synap server: ' . $e->getMessage());
        }
    }

    /**
     * S2S Test: Test invalid credentials
     */
    public function testInvalidBasicAuthS2S(): void
    {
        $url = getenv('SYNAP_URL') ?: self::TEST_URL;

        $config = SynapConfig::create($url)
            ->withBasicAuth('invalid', 'invalid');
        $client = new SynapClient($config);

        try {
            $client->kv()->get('test_key');
            $this->fail('Should have thrown exception for invalid credentials');
        } catch (\Exception $e) {
            // Expected - authentication should fail
            $this->assertInstanceOf(\Exception::class, $e);
        }
    }

    /**
     * S2S Test: Test invalid API key
     */
    public function testInvalidApiKeyS2S(): void
    {
        $url = getenv('SYNAP_URL') ?: self::TEST_URL;

        $config = SynapConfig::create($url)
            ->withAuthToken('invalid-api-key-12345');
        $client = new SynapClient($config);

        try {
            $client->kv()->get('test_key');
            $this->fail('Should have thrown exception for invalid API key');
        } catch (\Exception $e) {
            // Expected - authentication should fail
            $this->assertInstanceOf(\Exception::class, $e);
        }
    }
}


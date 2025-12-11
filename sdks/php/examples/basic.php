<?php

declare(strict_types=1);

require_once __DIR__ . '/../vendor/autoload.php';

use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

// Create client
$config = SynapConfig::create('http://localhost:15500')
    ->withTimeout(30);

$client = new SynapClient($config);

echo "=== Synap PHP SDK - Basic Example ===\n\n";

// Key-Value operations
echo "1. Key-Value Store:\n";
$client->kv()->set('user:1', 'John Doe');
$value = $client->kv()->get('user:1');
echo "  - Set user:1 = John Doe\n";
echo "  - Get user:1 = {$value}\n";

// With TTL
$client->kv()->set('session:abc', 'token123', 3600);
echo "  - Set session with 3600s TTL\n";

// Atomic operations
$counter = $client->kv()->incr('visitors');
echo "  - Increment visitors = {$counter}\n";

echo "\n";

// Queue operations
echo "2. Message Queue:\n";
$client->queue()->createQueue('tasks');
$msgId = $client->queue()->publish('tasks', ['task' => 'process-video'], 9);
echo "  - Published message: {$msgId}\n";

$message = $client->queue()->consume('tasks', 'worker-1');
if ($message) {
    echo "  - Consumed message: {$message->id}\n";
    $client->queue()->ack('tasks', $message->id);
    echo "  - Acknowledged message\n";
}

echo "\n";

// Stream operations
echo "3. Event Stream:\n";
$client->stream()->createRoom('chat-room-1');
$offset = $client->stream()->publish('chat-room-1', 'message', [
    'user' => 'alice',
    'text' => 'Hello!',
]);
echo "  - Published event at offset: {$offset}\n";

$events = $client->stream()->consume('chat-room-1', 0, 10);
echo "  - Consumed " . count($events) . " events\n";

echo "\n";

// Pub/Sub operations
echo "4. Pub/Sub:\n";
$client->pubsub()->subscribeTopics('user-123', ['notifications.*']);
$delivered = $client->pubsub()->publish('notifications.email', [
    'to' => 'user@example.com',
    'subject' => 'Welcome',
]);
echo "  - Published to {$delivered} subscribers\n";

echo "\n✅ All operations completed successfully!\n";

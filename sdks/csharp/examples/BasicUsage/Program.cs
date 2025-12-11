using Synap.SDK;

Console.WriteLine("=== Synap C# SDK - Basic Usage Example ===\n");

// Create client
var config = SynapConfig.Create("http://localhost:15500");
using var client = new SynapClient(config);

try
{
    // ===== Key-Value Store =====
    Console.WriteLine("📦 Key-Value Store Operations:");

    await client.KV.SetAsync("user:1", "John Doe");
    var userName = await client.KV.GetAsync<string>("user:1");
    Console.WriteLine($"  User: {userName}");

    await client.KV.SetAsync("session:abc", new { userId = "123", token = "xyz" }, ttl: 3600);
    var session = await client.KV.GetAsync("session:abc");
    Console.WriteLine($"  Session: {session}");

    var counter = await client.KV.IncrAsync("visits", 1);
    Console.WriteLine($"  Visits: {counter}");

    var keys = await client.KV.ScanAsync("user:*", limit: 10);
    Console.WriteLine($"  Found {keys.Count} user keys\n");

    // ===== Message Queues =====
    Console.WriteLine("📨 Message Queue Operations:");

    await client.Queue.CreateQueueAsync("tasks");
    var messageId = await client.Queue.PublishAsync("tasks", new
    {
        action = "encode-video",
        file = "video.mp4"
    }, priority: 9);
    Console.WriteLine($"  Published message: {messageId}");

    var message = await client.Queue.ConsumeAsync("tasks", "worker-1");
    if (message is not null)
    {
        Console.WriteLine($"  Consumed message: {message.Id}");
        Console.WriteLine($"  Priority: {message.Priority}");
        Console.WriteLine($"  Payload: {message.Payload}");
        await client.Queue.AckAsync("tasks", message.Id);
        Console.WriteLine("  Message acknowledged\n");
    }

    // ===== Event Streams =====
    Console.WriteLine("📡 Event Stream Operations:");

    await client.Stream.CreateRoomAsync("events");
    var offset = await client.Stream.PublishAsync("events", "user.created", new
    {
        userId = "456",
        name = "Alice",
        email = "alice@example.com"
    });
    Console.WriteLine($"  Published event at offset: {offset}");

    var events = await client.Stream.ReadAsync("events", offset: 0, limit: 10);
    Console.WriteLine($"  Read {events.Count} events:");
    foreach (var evt in events)
    {
        Console.WriteLine($"    - {evt.Event} (offset: {evt.Offset})");
    }
    Console.WriteLine();

    // ===== Pub/Sub =====
    Console.WriteLine("🔔 Pub/Sub Operations:");

    await client.PubSub.SubscribeTopicsAsync("subscriber-1", new List<string>
    {
        "notifications.*",
        "alerts.#"
    });
    Console.WriteLine("  Subscribed to topics");

    var delivered = await client.PubSub.PublishAsync("notifications.email", new
    {
        to = "user@example.com",
        subject = "Welcome to Synap!",
        body = "Thanks for trying our SDK"
    });
    Console.WriteLine($"  Message delivered to {delivered} subscribers\n");

    // ===== Statistics =====
    Console.WriteLine("📊 Statistics:");

    var kvStats = await client.KV.StatsAsync();
    Console.WriteLine($"  KV Store: {kvStats.Count} properties");

    var queueStats = await client.Queue.StatsAsync("tasks");
    Console.WriteLine($"  Queue 'tasks': {queueStats.Count} properties");

    var streamStats = await client.Stream.StatsAsync("events");
    Console.WriteLine($"  Stream 'events': {streamStats.Count} properties");

    Console.WriteLine("\n✅ All operations completed successfully!");
}
catch (Exception ex)
{
    Console.WriteLine($"\n❌ Error: {ex.Message}");
    Console.WriteLine($"   Stack: {ex.StackTrace}");
}


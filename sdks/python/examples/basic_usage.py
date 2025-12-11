"""Basic usage example for Synap Python SDK."""

import asyncio

from synap_sdk import SynapClient, SynapConfig


async def main() -> None:
    """Run basic usage examples."""
    print("=== Synap Python SDK - Basic Usage Example ===\n")

    # Create client
    config = SynapConfig.create("http://localhost:15500")
    async with SynapClient(config) as client:
        try:
            # ===== Key-Value Store =====
            print("📦 Key-Value Store Operations:")

            await client.kv.set("user:1", "John Doe")
            user_name = await client.kv.get("user:1")
            print(f"  User: {user_name}")

            await client.kv.set("session:abc", {"userId": "123", "token": "xyz"}, ttl=3600)
            session = await client.kv.get("session:abc")
            print(f"  Session: {session}")

            counter = await client.kv.incr("visits", 1)
            print(f"  Visits: {counter}")

            keys = await client.kv.scan("user:*", limit=10)
            print(f"  Found {len(keys)} user keys\n")

            # ===== Message Queues =====
            print("📨 Message Queue Operations:")

            await client.queue.create_queue("tasks")
            message_id = await client.queue.publish(
                "tasks",
                {"action": "encode-video", "file": "video.mp4"},
                priority=9,
            )
            print(f"  Published message: {message_id}")

            message = await client.queue.consume("tasks", "worker-1")
            if message:
                print(f"  Consumed message: {message.id}")
                print(f"  Priority: {message.priority}")
                print(f"  Payload: {message.payload}")
                await client.queue.ack("tasks", message.id)
                print("  Message acknowledged\n")

            # ===== Event Streams =====
            print("📡 Event Stream Operations:")

            await client.stream.create_room("events")
            offset = await client.stream.publish(
                "events",
                "user.created",
                {"userId": "456", "name": "Alice", "email": "alice@example.com"},
            )
            print(f"  Published event at offset: {offset}")

            events = await client.stream.read("events", offset=0, limit=10)
            print(f"  Read {len(events)} events:")
            for evt in events:
                print(f"    - {evt.event} (offset: {evt.offset})")
            print()

            # ===== Pub/Sub =====
            print("🔔 Pub/Sub Operations:")

            await client.pubsub.subscribe_topics("subscriber-1", ["notifications.*", "alerts.#"])
            print("  Subscribed to topics")

            delivered = await client.pubsub.publish(
                "notifications.email",
                {
                    "to": "user@example.com",
                    "subject": "Welcome to Synap!",
                    "body": "Thanks for trying our SDK",
                },
            )
            print(f"  Message delivered to {delivered} subscribers\n")

            # ===== Statistics =====
            print("📊 Statistics:")

            kv_stats = await client.kv.stats()
            print(f"  KV Store: {len(kv_stats)} properties")

            queue_stats = await client.queue.stats("tasks")
            print(f"  Queue 'tasks': {len(queue_stats)} properties")

            stream_stats = await client.stream.stats("events")
            print(f"  Stream 'events': {len(stream_stats)} properties")

            print("\n✅ All operations completed successfully!")

        except Exception as e:
            print(f"\n❌ Error: {e}")
            import traceback

            traceback.print_exc()


if __name__ == "__main__":
    asyncio.run(main())

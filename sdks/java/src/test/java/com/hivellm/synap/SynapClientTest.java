package com.hivellm.synap;

import com.hivellm.synap.types.Message;
import com.hivellm.synap.types.QueueStats;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import org.junit.jupiter.api.*;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests for the Synap Java SDK using JDK's built-in HTTP server as a mock.
 */
class SynapClientTest {

    private static HttpServer server;
    private static SynapClient client;
    private static final AtomicReference<String> lastRequestBody = new AtomicReference<>();
    private static final AtomicReference<String> nextResponse = new AtomicReference<>();

    @BeforeAll
    static void setUp() throws IOException {
        server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
        server.createContext("/api/v1/command", SynapClientTest::handleCommand);
        server.start();

        int port = server.getAddress().getPort();
        SynapConfig config = SynapConfig.builder("http://127.0.0.1:" + port)
                .timeout(Duration.ofSeconds(5))
                .build();
        client = new SynapClient(config);
    }

    @AfterAll
    static void tearDown() {
        if (client != null) client.close();
        if (server != null) server.stop(0);
    }

    private static void handleCommand(HttpExchange exchange) throws IOException {
        byte[] body = exchange.getRequestBody().readAllBytes();
        lastRequestBody.set(new String(body, StandardCharsets.UTF_8));

        String response = nextResponse.getAndSet(null);
        if (response == null) {
            response = "{\"success\":true,\"payload\":{}}";
        }

        byte[] responseBytes = response.getBytes(StandardCharsets.UTF_8);
        exchange.getResponseHeaders().set("Content-Type", "application/json");
        exchange.sendResponseHeaders(200, responseBytes.length);
        try (OutputStream os = exchange.getResponseBody()) {
            os.write(responseBytes);
        }
    }

    private void mockResponse(String json) {
        nextResponse.set(json);
    }

    private String getLastRequest() {
        return lastRequestBody.get();
    }

    // ── KV Tests ─────────────────────────────────────────────────────────────

    @Test
    void kvSet() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"success\":true}}");
        client.kv().set("key1", "value1");
        assertTrue(getLastRequest().contains("\"command\":\"kv.set\""));
        assertTrue(getLastRequest().contains("\"key\":\"key1\""));
        assertTrue(getLastRequest().contains("\"value\":\"value1\""));
    }

    @Test
    void kvSetWithTtl() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"success\":true}}");
        client.kv().set("key2", "value2", 60);
        assertTrue(getLastRequest().contains("\"ttl\":60"));
    }

    @Test
    void kvGetFound() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":\"hello\"}");
        String value = client.kv().get("mykey");
        assertEquals("hello", value);
        assertTrue(getLastRequest().contains("\"command\":\"kv.get\""));
    }

    @Test
    void kvGetNotFound() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":null}");
        String value = client.kv().get("missing");
        assertNull(value);
    }

    @Test
    void kvDelete() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"deleted\":true}}");
        boolean deleted = client.kv().delete("key1");
        assertTrue(deleted);
        assertTrue(getLastRequest().contains("\"command\":\"kv.del\""));
    }

    @Test
    void kvExists() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"exists\":true}}");
        assertTrue(client.kv().exists("key1"));
    }

    @Test
    void kvIncr() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"value\":42}}");
        long val = client.kv().incr("counter");
        assertEquals(42, val);
    }

    @Test
    void kvDecr() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"value\":9}}");
        long val = client.kv().decr("counter");
        assertEquals(9, val);
    }

    // ── Queue Tests ──────────────────────────────────────────────────────────

    @Test
    void queueCreate() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{}}");
        client.queue().create("test-q", 1000, 30);
        assertTrue(getLastRequest().contains("\"command\":\"queue.create\""));
        assertTrue(getLastRequest().contains("\"name\":\"test-q\""));
    }

    @Test
    void queuePublish() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"message_id\":\"msg-123\"}}");
        String id = client.queue().publish("test-q", "hello".getBytes(), 5, 3);
        assertEquals("msg-123", id);
        assertTrue(getLastRequest().contains("\"command\":\"queue.publish\""));
    }

    @Test
    void queueConsumeMessage() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"message\":{\"id\":\"msg-1\",\"payload\":[104,105],\"priority\":5,\"retry_count\":0,\"max_retries\":3}}}");
        Message msg = client.queue().consume("test-q", "worker-1");
        assertNotNull(msg);
        assertEquals("msg-1", msg.getId());
        assertEquals(5, msg.getPriority());
    }

    @Test
    void queueConsumeEmpty() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"message\":null}}");
        Message msg = client.queue().consume("test-q", "worker-1");
        assertNull(msg);
    }

    @Test
    void queueAck() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{}}");
        client.queue().ack("test-q", "msg-1");
        assertTrue(getLastRequest().contains("\"command\":\"queue.ack\""));
    }

    @Test
    void queueNack() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{}}");
        client.queue().nack("test-q", "msg-1");
        assertTrue(getLastRequest().contains("\"command\":\"queue.nack\""));
    }

    @Test
    void queueStats() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"depth\":10,\"consumers\":2,\"published\":100,\"consumed\":90,\"acked\":85,\"nacked\":5,\"dead_lettered\":0}}");
        QueueStats stats = client.queue().stats("test-q");
        assertEquals(10, stats.getDepth());
        assertEquals(100, stats.getPublished());
    }

    @Test
    void queueList() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"queues\":[\"q1\",\"q2\"]}}");
        List<String> queues = client.queue().list();
        assertEquals(2, queues.size());
        assertTrue(queues.contains("q1"));
    }

    @Test
    void queueDelete() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{}}");
        client.queue().delete("test-q");
        assertTrue(getLastRequest().contains("\"command\":\"queue.delete\""));
    }

    // ── Hash Tests ───────────────────────────────────────────────────────────

    @Test
    void hashSet() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"created\":true}}");
        boolean created = client.hash().set("h1", "field1", "value1");
        assertTrue(created);
        assertTrue(getLastRequest().contains("\"command\":\"hash.set\""));
    }

    @Test
    void hashGet() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"found\":true,\"value\":\"val\"}}");
        String val = client.hash().get("h1", "field1");
        assertEquals("val", val);
    }

    @Test
    void hashGetAll() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"fields\":{\"a\":\"1\",\"b\":\"2\"}}}");
        Map<String, String> all = client.hash().getAll("h1");
        assertEquals("1", all.get("a"));
        assertEquals("2", all.get("b"));
    }

    @Test
    void hashExists() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"exists\":true}}");
        assertTrue(client.hash().exists("h1", "field1"));
    }

    // ── List Tests ───────────────────────────────────────────────────────────

    @Test
    void listLPush() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"len\":3}}");
        int len = client.list().lpush("l1", "a", "b", "c");
        assertEquals(3, len);
        assertTrue(getLastRequest().contains("\"command\":\"list.lpush\""));
    }

    @Test
    void listRange() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"values\":[\"a\",\"b\",\"c\"]}}");
        List<String> items = client.list().range("l1", 0, -1);
        assertEquals(3, items.size());
        assertEquals("a", items.get(0));
    }

    @Test
    void listLen() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"len\":5}}");
        int len = client.list().len("l1");
        assertEquals(5, len);
    }

    @Test
    void listLPop() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"values\":[\"x\"]}}");
        List<String> popped = client.list().lpop("l1", 1);
        assertEquals(1, popped.size());
        assertEquals("x", popped.get(0));
    }

    // ── Set Tests ────────────────────────────────────────────────────────────

    @Test
    void setAdd() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"added\":2}}");
        int added = client.set().add("s1", "a", "b");
        assertEquals(2, added);
        assertTrue(getLastRequest().contains("\"command\":\"set.add\""));
    }

    @Test
    void setMembers() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"members\":[\"a\",\"b\",\"c\"]}}");
        java.util.Set<String> members = client.set().members("s1");
        assertEquals(3, members.size());
        assertTrue(members.contains("a"));
    }

    @Test
    void setIsMember() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"is_member\":true}}");
        assertTrue(client.set().isMember("s1", "a"));
    }

    @Test
    void setCard() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"count\":5}}");
        assertEquals(5, client.set().card("s1"));
    }

    // ── Stream Tests ─────────────────────────────────────────────────────────

    @Test
    void streamCreate() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{}}");
        client.stream().create("room1", 1000);
        assertTrue(getLastRequest().contains("\"command\":\"stream.create\""));
    }

    @Test
    void streamPublish() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"offset\":42}}");
        long offset = client.stream().publish("room1", "msg", "{\"n\":1}");
        assertEquals(42, offset);
    }

    @Test
    void streamList() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"rooms\":[\"r1\",\"r2\"]}}");
        List<String> rooms = client.stream().list();
        assertEquals(2, rooms.size());
    }

    // ── PubSub Tests ─────────────────────────────────────────────────────────

    @Test
    void pubsubPublish() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"subscribers_matched\":3}}");
        int matched = client.pubsub().publish("topic1", "{\"data\":1}", 5);
        assertEquals(3, matched);
        assertTrue(getLastRequest().contains("\"command\":\"pubsub.publish\""));
    }

    @Test
    void pubsubSubscribe() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"subscriber_id\":\"sub-1\",\"topics\":[\"t1\"]}}");
        String subId = client.pubsub().subscribe("sub-1", List.of("t1"));
        assertEquals("sub-1", subId);
    }

    @Test
    void pubsubListTopics() throws SynapException {
        mockResponse("{\"success\":true,\"payload\":{\"topics\":[\"t1\",\"t2\"]}}");
        List<String> topics = client.pubsub().listTopics();
        assertEquals(2, topics.size());
    }

    // ── Error Handling ───────────────────────────────────────────────────────

    @Test
    void serverErrorThrowsSynapException() {
        mockResponse("{\"success\":false,\"error\":\"key not found\",\"payload\":null}");
        assertThrows(SynapException.class, () -> client.kv().get("nope"));
    }
}

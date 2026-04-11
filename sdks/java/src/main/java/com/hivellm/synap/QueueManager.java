package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.hivellm.synap.types.Message;
import com.hivellm.synap.types.QueueStats;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Message queue operations.
 *
 * <p>Obtain an instance via {@link SynapClient#queue()}.
 */
public final class QueueManager {

    private final SynapClient client;

    QueueManager(SynapClient client) {
        this.client = client;
    }

    /**
     * Creates a new queue.
     *
     * @param name           queue name
     * @param maxDepth       maximum number of messages the queue holds (0 = unlimited)
     * @param ackDeadlineSecs seconds a consumer has to acknowledge before re-delivery
     * @throws SynapException on network or server error
     */
    public void create(String name, long maxDepth, long ackDeadlineSecs) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("name", name);
        payload.put("max_depth", maxDepth);
        payload.put("ack_deadline_secs", ackDeadlineSecs);
        client.sendCommand("queue.create", payload);
    }

    /**
     * Publishes a message to the named queue.
     *
     * @param name       queue name
     * @param data       raw message payload bytes
     * @param priority   message priority (0–9; higher = more important)
     * @param maxRetries maximum delivery attempts before the message is dead-lettered
     * @return the server-assigned message identifier
     * @throws SynapException on network or server error
     */
    public String publish(String name, byte[] data, int priority, int maxRetries) {
        // The server expects payload as an array of unsigned ints (byte values 0-255).
        int[] intPayload = new int[data.length];
        for (int i = 0; i < data.length; i++) {
            intPayload[i] = data[i] & 0xFF;
        }

        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("queue", name);
        payload.put("payload", intPayload);
        payload.put("priority", priority);
        payload.put("max_retries", maxRetries);

        JsonNode responsePayload = client.sendCommand("queue.publish", payload);

        JsonNode msgIdNode = responsePayload.get("message_id");
        return msgIdNode != null && !msgIdNode.isNull() ? msgIdNode.asText() : "";
    }

    /**
     * Consumes the next available message from the queue.
     *
     * @param name       queue name
     * @param consumerId unique identifier for this consumer instance
     * @return the next {@link Message}, or {@code null} if the queue is empty
     * @throws SynapException on network or server error
     */
    public Message consume(String name, String consumerId) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("queue", name);
        payload.put("consumer_id", consumerId);

        JsonNode responsePayload = client.sendCommand("queue.consume", payload);

        JsonNode messageNode = responsePayload.get("message");
        if (messageNode == null || messageNode.isNull()) {
            return null;
        }

        try {
            return client.mapper.treeToValue(messageNode, Message.class);
        } catch (Exception e) {
            throw SynapException.invalidResponse("Failed to deserialize Message: " + e.getMessage());
        }
    }

    /**
     * Acknowledges successful processing of the given message.
     *
     * @param name      queue name
     * @param messageId the ID returned by {@link #consume}
     * @throws SynapException on network or server error
     */
    public void ack(String name, String messageId) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("queue", name);
        payload.put("message_id", messageId);
        client.sendCommand("queue.ack", payload);
    }

    /**
     * Negatively acknowledges (requeues) the given message for re-delivery.
     *
     * @param name      queue name
     * @param messageId the ID returned by {@link #consume}
     * @throws SynapException on network or server error
     */
    public void nack(String name, String messageId) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("queue", name);
        payload.put("message_id", messageId);
        client.sendCommand("queue.nack", payload);
    }

    /**
     * Returns statistics for the named queue.
     *
     * @param name queue name
     * @return a {@link QueueStats} snapshot
     * @throws SynapException on network or server error
     */
    public QueueStats stats(String name) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("queue", name);
        JsonNode responsePayload = client.sendCommand("queue.stats", payload);

        try {
            return client.mapper.treeToValue(responsePayload, QueueStats.class);
        } catch (Exception e) {
            throw SynapException.invalidResponse("Failed to deserialize QueueStats: " + e.getMessage());
        }
    }

    /**
     * Lists the names of all queues currently registered on the server.
     *
     * @return list of queue names (never null; may be empty)
     * @throws SynapException on network or server error
     */
    public List<String> list() {
        JsonNode responsePayload = client.sendCommand("queue.list");

        List<String> result = new ArrayList<>();
        JsonNode queuesNode = responsePayload.get("queues");
        if (queuesNode != null && queuesNode.isArray()) {
            for (JsonNode item : queuesNode) {
                if (!item.isNull()) {
                    result.add(item.asText());
                }
            }
        }
        return result;
    }

    /**
     * Deletes the named queue and all its pending messages.
     *
     * @param name queue name
     * @throws SynapException on network or server error
     */
    public void delete(String name) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("queue", name);
        client.sendCommand("queue.delete", payload);
    }
}

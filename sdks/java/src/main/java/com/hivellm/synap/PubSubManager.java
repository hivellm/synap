package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Pub/Sub operations.
 *
 * <p>Obtain an instance via {@link SynapClient#pubsub()}.
 */
public final class PubSubManager {

    private final SynapClient client;

    PubSubManager(SynapClient client) {
        this.client = client;
    }

    /**
     * Publishes data to the named topic.
     *
     * @param topic    topic name
     * @param data     payload to publish (any JSON-serializable object)
     * @param priority message priority (0-9; higher = delivered first)
     * @return the number of subscribers that matched the topic pattern
     * @throws SynapException on network or server error
     */
    public int publish(String topic, Object data, int priority) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("topic", topic);
        payload.put("payload", data);
        payload.put("priority", priority);

        JsonNode responsePayload = client.sendCommand("pubsub.publish", payload);

        JsonNode smNode = responsePayload.get("subscribers_matched");
        return smNode != null ? smNode.asInt(0) : 0;
    }

    /**
     * Registers a subscription for the given topics.
     *
     * @param subscriberId unique identifier for this subscriber
     * @param topics       list of topic patterns to subscribe to
     * @return the server-assigned subscription identifier
     * @throws SynapException on network or server error
     */
    public String subscribe(String subscriberId, List<String> topics) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("subscriber_id", subscriberId);
        payload.put("topics", topics);

        JsonNode responsePayload = client.sendCommand("pubsub.subscribe", payload);

        JsonNode subIdNode = responsePayload.get("subscription_id");
        return subIdNode != null && !subIdNode.isNull() ? subIdNode.asText() : subscriberId;
    }

    /**
     * Removes the given topic subscriptions for the subscriber.
     *
     * @param subscriberId the subscriber identifier used when subscribing
     * @param topics       the topic patterns to unsubscribe from
     * @throws SynapException on network or server error
     */
    public void unsubscribe(String subscriberId, List<String> topics) {
        Map<String, Object> payload = SynapClient.newPayload();
        payload.put("subscriber_id", subscriberId);
        payload.put("topics", topics);
        client.sendCommand("pubsub.unsubscribe", payload);
    }

    /**
     * Lists all active topic patterns that have at least one subscriber.
     *
     * @return list of topic pattern strings (never null; may be empty)
     * @throws SynapException on network or server error
     */
    public List<String> listTopics() {
        JsonNode responsePayload = client.sendCommand("pubsub.topics");

        List<String> result = new ArrayList<>();
        JsonNode topicsNode = responsePayload.get("topics");
        if (topicsNode != null && topicsNode.isArray()) {
            for (JsonNode item : topicsNode) {
                if (!item.isNull()) {
                    result.add(item.asText());
                }
            }
        }
        return result;
    }
}

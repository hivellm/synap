# Scrape-time broker gauge snapshot with reset

**Category**: observability
**Tags**: metrics, prometheus, streams, consumer-groups, issue-196

## Description

To expose live broker state (stream length, consumer lag, queue depth) as Prometheus gauges without instrumenting every mutation, inject State<AppState> into the /metrics handler and, on each scrape, call reset_broker_gauges() then repopulate every IntGaugeVec by enumerating the managers (list_rooms/room_stats, list_topics/topic_stats, list_groups/get_offset, list_queues/stats). The reset is essential: gauges keep the last value per label set forever, so a deleted stream/group/queue would otherwise report a frozen stale value. Consumer lag = partition end_offset (high-water-mark) − committed offset, saturating at 0.

## Example

pub async fn update_broker_metrics(state: &AppState) {
    crate::metrics::reset_broker_gauges();
    if let Some(sm) = &state.stream_manager {
        for room in sm.list_rooms().await {
            if let Ok(s) = sm.room_stats(&room).await {
                set_stream_gauges(&room, s.message_count as i64, s.max_offset as i64, s.subscriber_count as i64);
            }
        }
    } /* partitions, consumer groups (lag = end - committed), queues … */
}

## When to Use

Exposing current-state broker/registry metrics that are cheap to snapshot on demand.

## When NOT to Use

High-cardinality or expensive-to-compute state, or rates/counters (use counters incremented at the event site instead).

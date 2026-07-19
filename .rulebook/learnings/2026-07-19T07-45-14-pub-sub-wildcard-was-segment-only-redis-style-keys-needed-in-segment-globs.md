# Pub/sub wildcard was segment-only; Redis-style keys needed in-segment globs
**Source**: manual
**Date**: 2026-07-19
**Related Task**: phase22_kv-watch-server-endpoints
**Tags**: kv-watch, pubsub, wildcards, phase22, analysis:kv-watch-observable
Analysis F-011 claimed wildcard KV watch was free via PubSubRouter's existing matching. False: the matcher is MQTT-style (split on '.', '*' matches one whole segment), so 'user:*' compiled to Exact("user:*") and matched nothing — keys use ':' and never split. Fixed with SegmentMatcher::Glob (pieces around '*', prefix/suffix-anchored greedy match), purely additive since embedded-star patterns previously only matched a literal-star topic. Lesson: validate an analysis claim of 'existing infra covers it' at the exact syntax level (delimiter, segmenting) before promising the feature; also KV.WATCH exists (not WATCH) because WATCH is the transaction command with an indistinguishable arg shape.
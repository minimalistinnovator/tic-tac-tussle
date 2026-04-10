//! Redpanda adapter — `EventPublisher` + parallel consumer groups.
//!
//! Parallel Receiver Topology:
//!   Every entry in CONSUMER_GROUPS is an independent Kafka consumer group.
//!   All groups read the same topic; messages funnel into one crossbeam channel.
//!   Add an observer: append one string to CONSUMER_GROUPS. Zero other changes.

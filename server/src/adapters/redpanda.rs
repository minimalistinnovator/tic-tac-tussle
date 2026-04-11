//! Redpanda adapter — `EventPublisher` + parallel consumer groups.
//!
//! Parallel Receiver Topology:
//!   Every entry in CONSUMER_GROUPS is an independent Kafka consumer group.
//!   All groups read the same topic; messages funnel into one crossbeam channel.
//!   Add an observer: append one string to CONSUMER_GROUPS. Zero other changes.

use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::{ClientContext, DefaultClientContext};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{
    BaseConsumer, CommitMode, Consumer, ConsumerContext, Rebalance, StreamConsumer,
};
use rdkafka::error::KafkaResult;
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use store::{EventPublisher, GameCommand, GameEventEnvelope};
use tokio::runtime::Runtime;
use tracing::{debug, error, info, warn};

const CONSUMER_GROUPS: &[&str] = &["game-server-loop", "game-audit-log"];

#[derive(Clone)]
pub struct BrokerConfig {
    pub brokers: String,
    pub events_topic: String,
    pub commands_topic: String,
}

pub struct BrokerCommand(pub GameCommand);

// ── Publisher ─────────────────────────────────────────────────────────────────

pub struct RedpandaPublisher {
    producer: FutureProducer,
    events_topic: String,
    commands_topic: String,
    rt: Runtime,
}

impl RedpandaPublisher {
    pub fn new(cfg: &BrokerConfig) -> Result<Self> {
        let producer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.brokers)
            .set("message.timeout.ms", "5000")
            .set("enable.idempotence", "true")
            .set("acks", "all")
            .create::<FutureProducer>()
            .context("FutureProducer")?;
        let rt = Runtime::new().context("publisher runtime")?;
        rt.block_on(ensure_topics(
            &build_admin(&cfg.brokers),
            &[&cfg.events_topic, &cfg.commands_topic],
        ));
        Ok(Self {
            producer,
            events_topic: cfg.events_topic.clone(),
            commands_topic: cfg.commands_topic.clone(),
            rt,
        })
    }
}

impl EventPublisher for RedpandaPublisher {
    fn publish_batch(&self, envelopes: Vec<GameEventEnvelope>) -> Result<()> {
        self.rt.block_on(async {
            for env in envelopes {
                let key = format!("{}/{}", env.game_id, env.sequence);
                let payload = serde_json::to_vec(&env).context("serialise envelope to JSON")?;
                let record = FutureRecord::to(&self.events_topic)
                    .key(&key)
                    .payload(&payload);
                match self.producer.send(record, Duration::from_secs(5)).await {
                    Ok(delivery) => debug!(
                        seq = env.sequence,
                        partition = delivery.partition,
                        offset = delivery.offset,
                        "published"
                    ),
                    Err((e, _)) => error!(%e, "publish failed"),
                }
            }
            Ok(())
        })
    }

    fn publish_command(&self, cmd: &GameCommand) -> Result<()> {
        self.rt.block_on(async {
            let payload = serde_json::to_vec(cmd).context("serialise command to JSON")?;
            let record: FutureRecord<(), [u8]> =
                FutureRecord::to(&self.commands_topic).payload(&payload);
            match self.producer.send(record, Duration::from_secs(5)).await {
                Ok(_) => debug!(?cmd, "published command"),
                Err((e, _)) => error!(%e, "publish command failed"),
            }
            Ok(())
        })
    }
}

// ── Parallel consumers ────────────────────────────────────────────────────────
pub fn spawn_parallel_consumers(cfg: BrokerConfig, cmd_tx: Sender<BrokerCommand>) {
    thread::Builder::new()
        .name("broker-consumers".into())
        .spawn(move || {
            Runtime::new().expect("consumer runtime").block_on(async {
                let handles: Vec<_> = CONSUMER_GROUPS
                    .iter()
                    .map(|&g| {
                        let c = cfg.clone();
                        let tx = cmd_tx.clone();
                        let g = g.to_string();
                        tokio::spawn(async move { consumer_loop(c, g, tx).await })
                    })
                    .collect();
                for h in handles {
                    let _ = h.await;
                }
            });
        })
        .expect("spawn broker thread");
}

struct LogCtx {
    group: String,
}
impl ClientContext for LogCtx {}
impl ConsumerContext for LogCtx {
    fn pre_rebalance(&self, _consumer: &BaseConsumer<Self>, rebalance: &Rebalance<'_>) {
        info!(group=%self.group, "pre-rebalance {rebalance:?}");
    }
    fn post_rebalance(&self, _consumer: &BaseConsumer<Self>, rebalance: &Rebalance<'_>) {
        info!(group=%self.group, "post-rebalance {rebalance:?}");
    }
    fn commit_callback(&self, result: KafkaResult<()>, _: &rdkafka::TopicPartitionList) {
        match result {
            Ok(_) => debug!(group=%self.group, "Offsets committed successfully"),
            Err(e) => warn!(group=%self.group, "Error committing offsets: {}", e),
        };
    }
}

async fn consumer_loop(cfg: BrokerConfig, group: String, cmd_tx: Sender<BrokerCommand>) {
    info!(group=%group, topic=%cfg.commands_topic, "consumer starting");
    let consumer: StreamConsumer<LogCtx> = ClientConfig::new()
        .set("bootstrap.servers", &cfg.brokers)
        .set("group.id", &group)
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "true")
        .set("auto.commit.interval.ms", "1000")
        .set("session.timeout.ms", "6000")
        .create_with_context(LogCtx {
            group: group.clone(),
        })
        .expect("StreamConsumer");
    consumer
        .subscribe(&[&cfg.commands_topic])
        .expect("subscribe");
    loop {
        match consumer.recv().await {
            Ok(msg) => {
                if let Some(payload) = msg.payload() {
                    let trimmed = payload
                        .iter()
                        .position(|&b| !b.is_ascii_whitespace())
                        .map(|start| &payload[start..])
                        .unwrap_or(payload);

                    let cmd_res = if trimmed.starts_with(b"{") || trimmed.starts_with(b"[") {
                        serde_json::from_slice::<GameCommand>(trimmed)
                            .map_err(|e| anyhow::anyhow!("JSON decode: {e}"))
                    } else {
                        bincode_next::decode_from_slice::<GameCommand, _>(
                            payload,
                            bincode_next::config::standard(),
                        )
                        .map(|(v, _)| v)
                        .map_err(|e| anyhow::anyhow!("Bincode decode: {e}"))
                    };

                    match cmd_res {
                        Ok(cmd) => {
                            debug!(group=%group, ?cmd, "received");
                            if cmd_tx.send(BrokerCommand(cmd)).is_err() {
                                warn!(group=%group, "game loop closed");
                                break;
                            }
                        }
                        Err(e) => {
                            let prefix = String::from_utf8_lossy(&payload[..payload.len().min(16)]);
                            warn!(group=%group, %e, payload_prefix=%prefix, "decode error");
                        }
                    }
                }
                let _ = consumer.commit_message(&msg, CommitMode::Async);
            }
            Err(e) => {
                error!(group=%group, %e, "error");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

fn build_admin(brokers: &str) -> AdminClient<DefaultClientContext> {
    ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()
        .expect("admin client")
}

async fn ensure_topics(admin: &AdminClient<DefaultClientContext>, topics: &[&str]) {
    let new: Vec<_> = topics
        .iter()
        .map(|t| NewTopic::new(t, 1, TopicReplication::Fixed(1)))
        .collect();
    if let Ok(results) = admin.create_topics(&new, &AdminOptions::new()).await {
        for r in results {
            match r {
                Ok(n) => info!("topic '{n}' ready"),
                Err((n, e)) => debug!("topic '{n}': {e}"),
            }
        }
    }
}

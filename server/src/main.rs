//! TicTacToe game server — 20 Hz game loop.
//!
//! The server coordinates the game state between multiple clients and an external
//! message broker (Redpanda). It runs a fixed-rate loop (20 Hz) to process:
//!
//! 1. Network clock synchronization and packet handling via `renet`.
//! 2. Client connection and disconnection events.
//! 3. Commands received directly from connected clients.
//! 4. Commands received from the message broker (potentially from other server instances).
//! 5. Flushing outbound network packets to clients.

use crate::adapters::redpanda::{
    BrokerCommand, BrokerConfig, RedpandaPublisher, spawn_parallel_consumers,
};
use crate::adapters::renet::RenetBroadcaster;
use crate::game_service::GameService;
use anyhow::{Context, Result};
use bincode_next::{config, decode_from_slice};
use crossbeam_channel::{Receiver, TryRecvError, bounded};
use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{
    NETCODE_USER_DATA_BYTES, NetcodeServerTransport, ServerAuthentication, ServerConfig,
};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use store::store::EventStore;
use store::{CommandEnvelope, GameCommand, GameId, PlayerId, TicTacTussleError};
use tracing::{info, warn};

mod adapters;
mod game_service;

/// The protocol identifier used to ensure clients and servers are compatible.
const PROTOCOL_ID: u64 = 0x5469_6354_6163;
/// The maximum number of clients allowed in a single game session.
const MAX_CLIENTS: usize = 2;
/// The target duration for each tick of the game loop (20 Hz).
const TICK: Duration = Duration::from_millis(50);

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("TicTacToe server starting");

    let kafka_brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:19092".into());

    let broker_cfg = BrokerConfig {
        brokers: kafka_brokers,
        events_topic: "game-events".into(),
        commands_topic: "game-commands".into(),
    };

    let addr: SocketAddr = "0.0.0.0:5000".parse()?;
    let socket = UdpSocket::bind(addr).context("bind UDP")?;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;

    let renet = Arc::new(Mutex::new(RenetServer::new(ConnectionConfig::default())));

    let mut transport = NetcodeServerTransport::new(
        ServerConfig {
            current_time: now,
            max_clients: MAX_CLIENTS,
            protocol_id: PROTOCOL_ID,
            public_addresses: vec![addr],
            authentication: ServerAuthentication::Unsecure,
        },
        socket,
    )
    .context("NetcodeServerTransport")?;

    info!("UDP listening on {addr}");

    let publisher = RedpandaPublisher::new(&broker_cfg)?;
    let broadcaster = RenetBroadcaster::new(Arc::clone(&renet));

    let game_id = GameId::new();
    let mut service = GameService::builder()
        .store(EventStore::new(game_id))
        .publisher(publisher)
        .broadcaster(broadcaster)
        .build();

    let (cmd_tx, cmd_rx) = bounded::<BrokerCommand>(256);
    spawn_parallel_consumers(broker_cfg, cmd_tx);

    let mut last = Instant::now();
    loop {
        let tick_start = Instant::now();
        let delta = tick_start - last;
        last = tick_start;

        {
            let mut srv = renet.lock().expect("renet poisoned");
            srv.update(delta);
            transport
                .update(delta, &mut srv)
                .context("transport update")?;
        }

        handle_connections(&renet, &mut transport, &mut service, game_id);
        drain_client_commands(&renet, &mut service, game_id);
        drain_broker_commands(&cmd_rx, &mut service);

        {
            let mut srv = renet.lock().expect("renet poisoned");
            transport.send_packets(&mut srv);
        }

        let elapsed = tick_start.elapsed();
        if elapsed < TICK {
            std::thread::sleep(TICK - elapsed);
        }
    }
}

// ── Game loop steps ───────────────────────────────────────────────────────────

/// Handles new client connections and disconnections.
///
/// When a client connects:
/// 1. A `JoinGame` command is created and processed.
/// 2. The new client is sent the current game state to catch up.
///
/// When a client disconnects:
/// 1. A `LeaveGame` command is created and processed.
fn handle_connections(
    renet: &Arc<Mutex<RenetServer>>,
    transport: &mut NetcodeServerTransport,
    service: &mut GameService,
    game_id: GameId,
) {
    let events: Vec<ServerEvent> = {
        let mut srv = renet.lock().expect("poisoned");
        std::iter::from_fn(|| srv.get_event()).collect()
    };
    for ev in events {
        match ev {
            ServerEvent::ClientConnected { client_id } => {
                let name = transport
                    .user_data(client_id)
                    .map(|ud| name_from_user_data(&ud))
                    .unwrap_or_else(|| format!("Player-{client_id}"));
                info!(%client_id, %name, "connected");
                if let Err(e) = service.catch_up(client_id) {
                    warn!(%e, "catch-up failed");
                }
                let cmd = GameCommand::JoinGame {
                    player_id: PlayerId(client_id),
                    name,
                };
                let cmd_env = CommandEnvelope::new(game_id, cmd);
                let _ = service.publish(store::BrokerMessage::Command(cmd_env.clone()));
                if let Err(e) = service.handle(cmd_env) {
                    warn!(%e, "JoinGame rejected");
                }
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!(%client_id, %reason, "disconnected");
                let cmd = GameCommand::LeaveGame {
                    player_id: PlayerId(client_id),
                };
                let cmd_env = CommandEnvelope::new(game_id, cmd);
                let _ = service.publish(store::BrokerMessage::Command(cmd_env.clone()));
                if let Err(e) = service.handle(cmd_env) {
                    warn!(%e, "LeaveGame rejected");
                }
            }
        }
    }
}
/// Drains and processes commands sent directly from connected clients.
fn drain_client_commands(
    renet: &Arc<Mutex<RenetServer>>,
    service: &mut GameService,
    game_id: GameId,
) {
    let messages: Vec<(u64, Vec<u8>)> = {
        let mut srv = renet.lock().expect("poisoned");
        let ids: Vec<u64> = srv.clients_id().into_iter().collect();
        let mut msgs = Vec::new();
        for id in ids {
            while let Some(b) = srv.receive_message(id, DefaultChannel::ReliableOrdered) {
                msgs.push((id, b.to_vec()));
            }
        }
        msgs
    };
    for (cid, raw) in messages {
        match decode_from_slice::<GameCommand, _>(&raw, config::standard()) {
            Ok((cmd, _)) => {
                let cmd_env = CommandEnvelope::new(game_id, cmd);
                let _ = service.publish(store::BrokerMessage::Command(cmd_env.clone()));
                if let Err(e) = service.handle(cmd_env) {
                    warn!(%cid, %e, "command rejected");
                }
            }
            Err(e) => warn!(%cid, %e, "decode error"),
        }
    }
}
/// Drains and processes commands received from the message broker.
///
/// Successfully processed commands are acknowledged (ACKed) to the broker.
/// Domain violations are also ACKed to avoid infinite retries of invalid commands.
/// Transient failures (like network issues) are NOT ACKed, allowing for redelivery.
fn drain_broker_commands(cmd_rx: &Receiver<BrokerCommand>, service: &mut GameService) {
    loop {
        match cmd_rx.try_recv() {
            Ok(BrokerCommand {
                command_envelope,
                ack,
            }) => {
                match service.handle(command_envelope) {
                    Ok(()) => ack.ack(),
                    Err(e) if e.downcast_ref::<TicTacTussleError>().is_some() => {
                        // Domain rule violation — retrying will never succeed.
                        // Ack so Redpanda advances past this message.
                        warn!(%e, "broker command permanently rejected — acking to skip");
                        ack.ack();
                    }
                    Err(e) => {
                        warn!(%e, "broker command transiently failed — dropping ack for redeliver");
                        // `ack` is dropped here — adapter sees Err(_) and does NOT commit.
                    }
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                warn!("broker channel closed");
                break;
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────
/// Extracts the player's name from the `Netcode` user data.
fn name_from_user_data(ud: &[u8; NETCODE_USER_DATA_BYTES]) -> String {
    let len = u64::from_le_bytes(ud[..8].try_into().unwrap()) as usize;
    let len = len.min(NETCODE_USER_DATA_BYTES - 8);
    String::from_utf8_lossy(&ud[8..8 + len]).into_owned()
}

//! TicTacToe game server — 20 Hz game loop.
//!
//! Each tick:
//!   1. Advance renet clocks; pump UDP.
//!   2. Handle connect / disconnect.
//!   3. Drain client GameCommands.
//!   4. Drain broker GameCommands (parallel consumer groups).
//!   5. Flush outbound UDP.

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
use store::{GameCommand, GameId, PlayerId};
use tracing::{info, warn};

mod adapters;
mod game_service;

const PROTOCOL_ID: u64 = 0x5469_6354_6163;
const MAX_CLIENTS: usize = 2;
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

    let mut service = GameService::builder()
        .store(EventStore::new(GameId::new()))
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

        handle_connections(&renet, &mut transport, &mut service);
        drain_client_commands(&renet, &mut service);
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

fn handle_connections(
    renet: &Arc<Mutex<RenetServer>>,
    transport: &mut NetcodeServerTransport,
    service: &mut GameService,
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
                let _ = service.publish_command(&cmd);
                if let Err(e) = service.handle(&cmd) {
                    warn!(%e, "JoinGame rejected");
                }
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!(%client_id, %reason, "disconnected");
                let cmd = GameCommand::LeaveGame {
                    player_id: PlayerId(client_id),
                };
                let _ = service.publish_command(&cmd);
                if let Err(e) = service.handle(&cmd) {
                    warn!(%e, "LeaveGame rejected");
                }
            }
        }
    }
}
fn drain_client_commands(renet: &Arc<Mutex<RenetServer>>, service: &mut GameService) {
    let messages: Vec<(u64, bytes::Bytes)> = {
        let mut srv = renet.lock().expect("poisoned");
        let ids: Vec<u64> = srv.clients_id().into_iter().collect();
        let mut msgs = Vec::new();
        for id in ids {
            while let Some(b) = srv.receive_message(id, DefaultChannel::ReliableOrdered) {
                msgs.push((id, b));
            }
        }
        msgs
    };
    for (cid, raw) in messages {
        match decode_from_slice::<GameCommand, _>(&raw, config::standard()) {
            Ok((cmd, _)) => {
                let _ = service.publish_command(&cmd);
                if let Err(e) = service.handle(&cmd) {
                    warn!(%cid, %e, "command rejected");
                }
            }
            Err(e) => warn!(%cid, %e, "decode error"),
        }
    }
}
fn drain_broker_commands(cmd_rx: &Receiver<BrokerCommand>, service: &mut GameService) {
    loop {
        match cmd_rx.try_recv() {
            Ok(BrokerCommand(cmd)) => {
                if let Err(e) = service.handle(&cmd) {
                    warn!(%e, "broker command rejected");
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
fn name_from_user_data(ud: &[u8; NETCODE_USER_DATA_BYTES]) -> String {
    let len = u64::from_le_bytes(ud[..8].try_into().unwrap()) as usize;
    let len = len.min(NETCODE_USER_DATA_BYTES - 8);
    String::from_utf8_lossy(&ud[8..8 + len]).into_owned()
}

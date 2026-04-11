//! Tic-tac-tussle Bevy client.
//!
//! Architecture:
//!   - Server is authoritative. Client NEVER self-applies moves.
//!   - Client sends GameCommand::PlaceTile to server over UDP.
//!   - Server validates, applies, and broadcasts GameEvent to all clients.
//!   - Client receives GameEvent, updates local GameState + PlayerPair, re-renders.
//!
//! Resources:
//!   - GameState — write model hydrated from received events
//!   - PlayerPair — built once both PlayerJoined events arrive
//!   - LocalPlayerId — this client's PlayerId (set on transport connection)

// ── Constants ─────────────────────────────────────────────────────────────────

use anyhow::Result;
use bevy::DefaultPlugins;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_renet::netcode::{ClientAuthentication, NetcodeClientPlugin, NetcodeClientTransport};
use bevy_renet::{RenetClient, RenetClientPlugin};
use bincode_next::{config, decode_from_slice, encode_to_vec};
use renet::{ConnectionConfig, DefaultChannel};
use std::net::{SocketAddr, UdpSocket};
use std::time::SystemTime;
use store::{
    EndGameReason, GameCommand, GameDecider, GameEvent, GameState, PlayerId, PlayerPair, Stage,
    Symbol, Tile,
};
const PROTOCOL_ID: u64 = 0x5469_6354_6163;
const CELL: f32 = 160.0;
const BOARD: f32 = CELL * 3.0;

// ── Resources ─────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
struct LocalPlayerId(Option<PlayerId>);

/// Accumulated received events — used to build PlayerPair lazily.
#[derive(Resource, Default)]
struct ReceivedEvents(Vec<GameEvent>);

/// Read model: available once both players have joined.
#[derive(Resource, Default)]
struct MaybePair(Option<PlayerPair>);

/// New type wrapper so we can implement `bevy::prelude::Message` on `GameEvent`
/// without polluting the domain crate with Bevy.
#[derive(Event, Clone, Message)]
struct BevyGameEvent(GameEvent);

#[derive(Resource, Default, Deref, DerefMut)]
struct BevyGameState(GameState);

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct HoverCell(usize);
#[derive(Component)]
struct Piece;

// ── Entry point ───────────────────────────────────────────────────────────────
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let username = args.get(1).cloned().unwrap_or_else(|| "Player".into());
    let server_str = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "127.0.0.1:5000".into());

    let mut app = App::new();
    app.insert_resource(Username(username.clone()));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: format!("TicTacToe — {username}"),
            resolution: WindowResolution::new(BOARD as u32, (BOARD + 60.0) as u32),
            resizable: false,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb_u8(30, 30, 30)))
    .add_plugins(RenetClientPlugin)
    .add_plugins(NetcodeClientPlugin)
    .insert_resource(RenetClient::new(ConnectionConfig::default()))
    .insert_resource(build_transport(&username, &server_str)?)
    // Domain resources
    .insert_resource(BevyGameState::default())
    .insert_resource(LocalPlayerId::default())
    .insert_resource(ReceivedEvents::default())
    .insert_resource(MaybePair::default())
    // Bevy events
    .add_message::<BevyGameEvent>()
    // Systems
    .add_systems(Startup, setup)
    .add_systems(PostUpdate, receive_server_events)
    .add_systems(
        Update,
        (
            set_local_player_id,
            update_pair,
            handle_input,
            render_pieces,
            render_hover,
            update_status,
        ),
    );

    app.run();

    Ok(())
}

// ── Setup ─────────────────────────────────────────────────────────────────────
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Grid lines
    let offsets = [-CELL / 2.0, CELL / 2.0];
    for &x in &offsets {
        commands.spawn((
            Sprite {
                color: Color::srgb_u8(80, 80, 80),
                custom_size: Some(Vec2::new(2.0, BOARD)),
                ..default()
            },
            Transform::from_xyz(x, 0.0, 0.0),
        ));
    }
    for &y in &offsets {
        commands.spawn((
            Sprite {
                color: Color::srgb_u8(80, 80, 80),
                custom_size: Some(Vec2::new(BOARD, 2.0)),
                ..default()
            },
            Transform::from_xyz(0.0, y, 0.0),
        ));
    }

    // Hover cells (invisible until cursor enters)
    for idx in 0..9usize {
        let (x, y) = cell_pos(idx);
        commands.spawn((
            Sprite {
                color: Color::NONE,
                custom_size: Some(Vec2::splat(CELL - 4.0)),
                ..default()
            },
            Transform::from_xyz(x, y, 0.5),
            HoverCell(idx),
        ));
    }

    // Status bar
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(60.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        children![(
            Text::new("Waiting for opponent…"),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::WHITE),
            StatusText,
        )],
    ));
}

// ── Systems ───────────────────────────────────────────────────────────────────

fn set_local_player_id(
    transport: Res<NetcodeClientTransport>,
    mut local: ResMut<LocalPlayerId>,
    mut client: ResMut<RenetClient>,
    username: Res<Username>,
) {
    if local.0.is_none() {
        let id = transport.client_id();
        local.0 = Some(PlayerId(id));

        // Join game immediately on connection
        let cmd = GameCommand::JoinGame {
            player_id: PlayerId(id),
            name: username.0.clone(),
        };
        let bytes = encode_to_vec(&cmd, config::standard()).expect("encode join");
        client.send_message(DefaultChannel::ReliableOrdered, bytes);
        info!("Sent JoinGame for {}", username.0);
    }
}

/// Drain UDP messages; apply each event to GameState; emit as Bevy event.
fn receive_server_events(
    mut client: ResMut<RenetClient>,
    mut state: ResMut<BevyGameState>,
    mut received: ResMut<ReceivedEvents>,
    mut writer: MessageWriter<BevyGameEvent>,
) {
    while let Some(raw) = client.receive_message(DefaultChannel::ReliableOrdered) {
        match decode_from_slice::<GameEvent, _>(&raw, config::standard()) {
            Ok((event, _)) => {
                **state = GameDecider::evolve(&state, &event);
                received.0.push(event.clone());
                writer.write(BevyGameEvent(event));
            }
            Err(e) => warn!("decode error: {e}"),
        }
    }
}

/// Build PlayerPair once both PlayerJoined events are in the log.
fn update_pair(received: Res<ReceivedEvents>, mut pair: ResMut<MaybePair>) {
    if pair.0.is_some() {
        return;
    }
    let joins: Vec<_> = received
        .0
        .iter()
        .filter_map(|e| {
            if let GameEvent::PlayerJoined { player_id, name } = e {
                Some((*player_id, name.clone()))
            } else {
                None
            }
        })
        .collect();
    if joins.len() == 2 {
        pair.0 = Some(PlayerPair::new(
            joins[0].0,
            joins[0].1.clone(),
            joins[1].0,
            joins[1].1.clone(),
        ));
    }
}

#[derive(Resource)]
struct Username(String);

/// Mouse click → PlaceTile command → server.
fn handle_input(
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<BevyGameState>,
    local: Res<LocalPlayerId>,
    mut client: ResMut<RenetClient>,
) {
    if state.stage != Stage::InGame {
        return;
    }
    let Some(my_id) = local.0 else { return };
    if state.active_player_id != my_id {
        return;
    }

    let window = windows.single().expect("primary window missing");
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let wx = cursor.x - window.width() / 2.0;
    let wy = window.height() / 2.0 - cursor.y;

    let col = ((wx + BOARD / 2.0) / CELL).floor() as isize;
    let row = ((BOARD / 2.0 - wy) / CELL).floor() as isize;

    if !(0..3).contains(&col) || !(0..3).contains(&row) {
        return;
    }
    let at = (row * 3 + col) as usize;

    if mouse.just_pressed(MouseButton::Left) {
        if state.board[at] != Tile::Empty {
            return;
        }
        let cmd = GameCommand::PlaceTile {
            player_id: my_id,
            at,
        };
        let bytes = encode_to_vec(&cmd, config::standard()).expect("encode");
        client.send_message(DefaultChannel::ReliableOrdered, bytes);
    }
}

/// Spawn a piece sprite for every TilePlaced event.
fn render_pieces(
    mut commands: Commands,
    mut events: MessageReader<BevyGameEvent>,
    local: Res<LocalPlayerId>,
    pair: Res<MaybePair>,
) {
    for event in events.read() {
        if let GameEvent::TilePlaced { player_id, at } = event.0 {
            let symbol = pair.0.as_ref().map(|p| p.symbol_of(player_id));
            let color = piece_color(symbol, player_id, local.0);
            let (x, y) = cell_pos(at);
            let symbol_text = match symbol {
                Some(Symbol::X) => "X",
                Some(Symbol::O) => "O",
                _ => "?",
            };
            commands
                .spawn((
                    Sprite {
                        color,
                        custom_size: Some(Vec2::splat(CELL * 0.55)),
                        ..default()
                    },
                    Transform::from_xyz(x, y, 1.0),
                    Piece,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text2d::new(symbol_text),
                        TextFont {
                            font_size: CELL * 0.4,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Transform::from_xyz(0.0, 0.0, 0.1),
                    ));
                });
        }
    }
}

/// Highlight the cell under the cursor when it's the local player's turn.
fn render_hover(
    windows: Query<&Window>,
    state: Res<BevyGameState>,
    local: Res<LocalPlayerId>,
    mut q: Query<(&HoverCell, &mut Sprite)>,
) {
    let window = windows.single().expect("primary window missing");
    let is_my_turn = local
        .0
        .map(|id| state.active_player_id == id && state.stage == Stage::InGame)
        .unwrap_or(false);

    let hovered = if is_my_turn {
        window.cursor_position().and_then(|c| {
            let wx = c.x - window.width() / 2.0;
            let wy = window.height() / 2.0 - c.y;
            let col = ((wx + BOARD / 2.0) / CELL).floor() as isize;
            let row = ((BOARD / 2.0 - wy) / CELL).floor() as isize;
            if (0..3).contains(&col) && (0..3).contains(&row) {
                Some((row * 3 + col) as usize)
            } else {
                None
            }
        })
    } else {
        None
    };

    for (cell, mut sprite) in q.iter_mut() {
        sprite
            .color
            .set_alpha(if hovered == Some(cell.0) { 0.2 } else { 0.0 });
    }
}

/// Update status bar text on each meaningful game event.
fn update_status(
    mut events: MessageReader<BevyGameEvent>,
    state: Res<BevyGameState>,
    local: Res<LocalPlayerId>,
    pair: Res<MaybePair>,
    mut q: Query<(&mut Text, &mut TextColor), With<StatusText>>,
) {
    for event in events.read() {
        let Ok((mut text, mut color)) = q.single_mut() else {
            continue;
        };
        let my_id = local.0.unwrap_or(PlayerId(0));

        match event.0 {
            GameEvent::GameStarted { .. } | GameEvent::TilePlaced { .. }
                if state.stage == Stage::InGame =>
            {
                if state.active_player_id == my_id {
                    **text = "Your turn!".into();
                    *color = TextColor(Color::srgb_u8(100, 210, 100));
                } else {
                    let opp = pair
                        .0
                        .as_ref()
                        .and_then(|p| p.name_of(state.active_player_id))
                        .unwrap_or("Opponent");
                    **text = format!("{opp}'s turn...");
                    *color = TextColor(Color::WHITE);
                }
            }
            GameEvent::GameEnded { reason } => {
                let (msg, clr) = match reason {
                    EndGameReason::PlayerWon { winner } => {
                        if winner == my_id {
                            ("You win! 🎉".into(), Color::srgb_u8(255, 215, 0))
                        } else {
                            let name = pair
                                .0
                                .as_ref()
                                .and_then(|p| p.name_of(winner))
                                .unwrap_or("Opponent");
                            (format!("{name} wins."), Color::srgb_u8(200, 80, 80))
                        }
                    }
                    EndGameReason::Draw => ("Draw!".into(), Color::srgb_u8(180, 180, 180)),
                    EndGameReason::PlayerLeft { .. } => {
                        ("Opponent left.".into(), Color::srgb_u8(160, 160, 160))
                    }
                };
                **text = msg;
                *color = TextColor(clr);
            }
            _ => {}
        }
    }
}

// ── Pure helpers ──────────────────────────────────────────────────────────────

fn cell_pos(idx: usize) -> (f32, f32) {
    let col = (idx % 3) as f32;
    let row = (idx / 3) as f32;
    let x = CELL * (col - 1.0);
    let y = CELL * (1.0 - row);
    (x, y)
}

fn piece_color(symbol: Option<Symbol>, player_id: PlayerId, local: Option<PlayerId>) -> Color {
    let is_me = local == Some(player_id);
    match (symbol, is_me) {
        (Some(Symbol::X), true) => Color::srgb_u8(70, 180, 230),
        (Some(Symbol::X), false) => Color::srgb_u8(230, 100, 70),
        (Some(Symbol::O), true) => Color::srgb_u8(70, 230, 150),
        (Some(Symbol::O), false) => Color::srgb_u8(230, 180, 70),
        (None, _) => Color::WHITE,
    }
}

fn build_transport(username: &str, server_addr: &str) -> Result<NetcodeClientTransport> {
    use renet_netcode::NETCODE_USER_DATA_BYTES;
    let addr: SocketAddr = server_addr.parse()?;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let client_id = now.as_millis() as u64;

    let mut ud = [0u8; NETCODE_USER_DATA_BYTES];
    let nb = username.as_bytes();
    let len = nb.len().min(NETCODE_USER_DATA_BYTES - 8);
    ud[..8].copy_from_slice(&(len as u64).to_le_bytes());
    ud[8..8 + len].copy_from_slice(&nb[..len]);

    Ok(NetcodeClientTransport::new(
        now,
        ClientAuthentication::Unsecure {
            client_id,
            protocol_id: PROTOCOL_ID,
            server_addr: addr,
            user_data: Some(ud),
        },
        socket,
    )?)
}

# Tic-Tac-Tussle

A small online tic-tac-toe game written in Rust, featuring a Redpanda-backed event store
and UDP-based networking.

## Prerequisites

```bash
# Verify these are installed
docker --version          # Docker 24+
docker compose version    # v2 plugin (not docker-compose v1)
cargo --version           # Rust 1.77+
rustup show               # should show stable toolchain
```

---

## Step 1 — Clone and Enter

```bash
git clone https://github.com/minimalistinnovator/tic-tac-tussle.git
cd tic-tac-tussle
```

---

## Step 2 — Start Redpanda + Server via Docker

```bash
# First time — build the server image and start everything
docker compose up --build

# Subsequent runs (no code changes)
docker compose up

# Run detached (background)
docker compose up --build -d
```

The commands above start three containers:

- `ttt-redpanda` — Redpanda broker on `:19092` (external) / `:9092` (internal)
- `ttt-console`  — Redpanda Console UI on `:8080`
- `ttt-server`   — your game server on UDP `:5000`

**Wait for this line before starting clients:**

```
ttt-server  | INFO server: UDP listening on 0.0.0.0:5000
```

---

## Step 3 — Start the Clients (Two Separate Terminals)

```bash
# Terminal 1 — Player Alice
RUST_LOG=client=debug cargo run --bin client -- Alice 127.0.0.1:5000

# Terminal 2 — Player Bob
RUST_LOG=client=debug cargo run --bin client -- Bob 127.0.0.1:5000
```

The server auto-starts the game the moment both clients connect.

---

## Step 4 — Monitor Redpanda Console (UI)

```bash
open http://localhost:8080        # macOS
xdg-open http://localhost:8080    # Linux
# Windows: just paste into browser
```

Inside the Console:

| Where                        | What you'll see                                                                                             |
|------------------------------|-------------------------------------------------------------------------------------------------------------|
| **Topics → `game-events`**   | Every `GameEventEnvelope` published — full message bytes, key (`game_id/seq`), partition, offset, timestamp |
| **Topics → `game-commands`** | Commands injected from external consumers                                                                   |
| **Consumer Groups**          | `game-server-loop` and `game-audit-log` lag and offset positions                                            |
| **Overview**                 | Broker health, throughput                                                                                   |

Click a message → **Deserialize as** → select `Binary` to see raw bytes (they're
`bincode` — not human-readable in the UI, but offset/key/timing are all visible).

---

## Step 5 — Monitor Logs

```bash
# All containers live
docker compose logs -f

# Just the server
docker compose logs -f server

# Just Redpanda
docker compose logs -f redpanda

# Server logs with timestamps
docker compose logs -f --timestamps server
```

The server emits structured `tracing` logs. Key lines to watch:

```
INFO  server: UDP listening on 0.0.0.0:5000
INFO  server: connected client_id=1 name="Alice"
INFO  server: connected client_id=2 name="Bob"
TRACE store::decider: command accepted cmd=JoinGame produced=2
INFO  server: broadcasted event=GameStarted
TRACE store::store: batch-appended seq=0
TRACE store::store: batch-appended seq=1
INFO  adapters::redpanda: published seq=0 partition=0 offset=0
```

---

## Step 6 — Inspect Redpanda via CLI (inside container)

```bash
# Shell into the Redpanda container
docker exec -it ttt-redpanda bash

# List topics
rpk topic list

# Consume game-events topic from beginning (raw — bincode bytes shown as hex)
rpk topic consume game-events --from-start

# Describe consumer group lag
rpk group describe game-server-loop
rpk group describe game-audit-log

# Topic metadata
rpk topic describe game-events
```

---

## Step 7 — Run Domain Tests (Zero Infrastructure)

```bash
# Pure domain — no Docker, no broker, no UDP needed
cargo test -p store

# Verbose output
cargo test -p store -- --nocapture

# Specific test
cargo test -p store decider::tests::win_ends_game -- --nocapture
```

---

## Tear Down

```bash
# Stop containers, keep volumes (Redpanda data persists)
docker compose down

# Stop AND wipe all Redpanda data (clean slate)
docker compose down -v
```

---

## Quick Reference Card

```
docker compose up --build -d          ← start everything
open http://localhost:8080             ← Redpanda Console
docker compose logs -f server          ← server logs
cargo run --bin client -- Alice 127.0.0.1:5000   ← client 1
cargo run --bin client -- Bob   127.0.0.1:5000   ← client 2
cargo test -p store                    ← domain tests
docker exec -it ttt-redpanda rpk topic consume game-events --from-start  ← raw events
docker compose down -v                 ← full teardown
```

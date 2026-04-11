# Tic-Tac-Tussle: Sequence Diagrams

These diagrams illustrate how the system processes specific user interactions in an event-sourced way.

## Player Joins and Game Starts

This interaction shows the transition from a Lobby state to an active Game state once the second player joins.

```mermaid
sequenceDiagram
    participant P1 as Player 1 (Alice)
    participant P2 as Player 2 (Bob)
    participant S as Server / Handler
    participant ES as Event Store
    participant RM as Read Model (GameState)

    P1->>S: GameCommand::JoinGame("Alice")
    S->>ES: Save: GameEvent::PlayerJoined("Alice")
    ES-->>RM: Project Event
    RM-->>P1: State: Lobby (Waiting for Opponent)

    P2->>S: GameCommand::JoinGame("Bob")
    S->>ES: Save: GameEvent::PlayerJoined("Bob")
    Note over S: 2 Players Joined!
    S->>ES: Save: GameEvent::GameStarted(first: Alice)
    ES-->>RM: Project both events
    RM-->>P1: State: InGame (Alice's Turn)
    RM-->>P2: State: InGame (Alice's Turn)
```

## Placing a Tile (Move & Validation)

This diagram shows how a move is processed and how it potentially triggers a win condition.

```mermaid
sequenceDiagram
    participant P as Player (Alice)
    participant S as Server / Handler
    participant ES as Event Store
    participant RM as Read Model (GameState)

    P->>S: GameCommand::PlaceTile(at: 4)
    Note over S: Validate: Is it Alice's turn? Is cell 4 empty?
    S->>ES: Save: GameEvent::TilePlaced(player: Alice, at: 4)

    Note over S: Check for Win Condition (3 in a row)
    alt Win Condition Met
        S->>ES: Save: GameEvent::GameEnded(Reason: PlayerWon(Alice))
    else No Winner yet
        Note over S: No further event
    end

    ES-->>RM: Project Events
    RM-->>P: Render updated board / result
```

## State Reconstruction (Event Sourcing)

When a player reloads the page or the server restarts, the state is rebuilt by "playing back" all the events from the store.

```mermaid
sequenceDiagram
    participant ES as Event Store
    participant RM as Read Model (Empty)

    ES->>RM: 1. Apply: PlayerJoined("Alice")
    ES->>RM: 2. Apply: PlayerJoined("Bob")
    ES->>RM: 3. Apply: GameStarted(first: Alice)
    ES->>RM: 4. Apply: TilePlaced(Alice, at: 4)
    ES->>RM: 5. Apply: TilePlaced(Bob, at: 0)
    Note right of RM: GameState is now fully current
```

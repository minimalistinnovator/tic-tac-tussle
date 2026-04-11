# Tic-Tac-Tussle: Big Picture Event Model

This diagram represents the "Big Picture" of the game's event-sourced lifecycle. It uses the [Event Modeling](https://eventmodeling.org/) pattern, visually represented with Mermaid.

```mermaid
graph LR
    subgraph "UI / UX (Passive Views)"
        UI_Lobby["Lobby Screen"]
        UI_Board["Active Game Board"]
        UI_End["Winner / Game Over Screen"]
    end

    subgraph "Commands (Intent - Blue)"
        direction TB
        C_Join["GameCommand::JoinGame"]
        C_Place["GameCommand::PlaceTile"]
        C_Leave["GameCommand::LeaveGame"]
    end

    subgraph "Event Store (Reality - Yellow)"
        direction TB
        E_Joined(("GameEvent::PlayerJoined"))
        E_Started(("GameEvent::GameStarted"))
        E_Placed(("GameEvent::TilePlaced"))
        E_Left(("GameEvent::PlayerLeft"))
        E_Ended(("GameEvent::GameEnded"))
    end

    subgraph "Read Models (Projections - Green)"
        direction TB
        RM_Lobby[("Lobby State")]
        RM_Game[("GameState (Board & Turns)")]
    end

    %% Lifecycle: Joining
    UI_Lobby --> C_Join
    C_Join --> E_Joined
    E_Joined --> RM_Lobby

    %% Lifecycle: Starting
    E_Joined -- "Second Player Joins" --> E_Started
    E_Started --> RM_Game

    %% Lifecycle: Playing
    RM_Game --> UI_Board
    UI_Board -- "Player Click" --> C_Place
    C_Place --> E_Placed
    E_Placed --> RM_Game

    %% Lifecycle: Ending (Win/Draw)
    E_Placed -- "Win Condition Met" --> E_Ended
    E_Ended --> RM_Game
    RM_Game --> UI_End

    %% Lifecycle: Leaving
    UI_Board --> C_Leave
    C_Leave --> E_Left
    E_Left --> E_Ended

    %% Styling
    classDef command fill:#3498db,stroke:#2980b9,color:#fff
    classDef event fill:#f1c40f,stroke:#f39c12,color:#000
    classDef readmodel fill:#2ecc71,stroke:#27ae60,color:#fff
    classDef ui fill:#ecf0f1,stroke:#bdc3c7,color:#000

    class C_Join,C_Place,C_Leave command
    class E_Joined,E_Started,E_Placed,E_Left,E_Ended event
    class RM_Lobby,RM_Game readmodel
    class UI_Lobby,UI_Board,UI_End ui
```

## Modeling Principles Used

1.  **Commands (Blue):** These represent a user's intent to change the system. They are validated against the current state before an event is emitted.
2.  **Events (Yellow):** These represent immutable facts that have already occurred. They are the "source of truth" (Event Sourcing).
3.  **Read Models (Green):** These are projections of the events. They are optimized for the UI to consume. In this project, `GameState` serves as the primary read model for the board.
4.  **Passive UI (Grey):** The UI reflects the current read model and dispatches commands. It does not contain business logic.

Maintainer

Valentin Heiserer <valentin@heiserer.de>

Features
- Deck construction and iteration
- Shuffling and dealing (4 players × 8 cards)
- Card and rank helpers (human-readable Display, point values)
- Game mode rules to determine trick winners (Sauspiel, Solo, Wenz, Geier,
  Bettel, Ramsch)
# schafkopf-logic

Logic and rules for the German card game Schafkopf. This crate provides types
and helpers for deck construction, common game modes and basic trick-taking
logic.

**Crate:** [`schafkopf-logic`](https://crates.io/crates/schafkopf-logic)

## Features

- Deck and card types (suits, ranks, cards) with Display implementations
- Shuffling and dealing (4 players × 8 cards)
- Rank point values and helpers
- Game mode logic to determine trick winners (Sauspiel, Solo, Wenz, Geier,
  Bettel, Ramsch)

## Quick example

```rust
use schafkopf_logic::deck::{Deck, Suit, Rank};
use schafkopf_logic::gamemode::Gamemode;

fn main() {
    // Create, shuffle and deal a deck
    let mut deck = Deck::new();
    deck.shuffle();
    let hands = deck.deal_4x8().expect("deck should contain 32 cards");

    // Form a sample trick from the first card of each hand and print it
    let trick = [&hands[0][0], &hands[1][0], &hands[2][0], &hands[3][0]];
    println!("Trick: {}, {}, {}, {}", trick[0], trick[1], trick[2], trick[3]);

    // Determine the winner for a sample game mode (Sauspiel with Schell)
    let winner = Gamemode::Sauspiel(Suit::Schell).winning_card(trick);
    println!("Winning card: {}", winner);
}
```

## Shared engine API

`game::Game` owns all four hands and the public trick state. It performs no I/O
and uses no random choices during transitions. Shuffle a `Deck` before passing
it to `Game::new`; an unshuffled deck is useful for deterministic tests.

```rust
use schafkopf_logic::{
    deck::{Deck, Suit},
    game::{Action, Game, GameConfig, PlayerId},
    gamemode::Gamemode,
};

let player = PlayerId::ALL[0];
let mut game = Game::new(
    GameConfig {
        mode: Gamemode::Solo(Suit::Herz),
        first_player: player,
    },
    Deck::new(),
)?;
let observation = game.observation(player);
let events = game.apply(player, Action::PlayCard(observation.hand[0]))?;
# Ok::<(), schafkopf_logic::game::GameError>(())
```

- `observation(player)` returns a detached snapshot containing that player's
  hand, public hand sizes, turn, played cards, and eventual trick winner.
- `apply(player, action)` validates before changing state and returns public
  events. Errors are structured `GameError` values. No mutation occurs on error.
- `next_player()`, `phase()`, and `completed_trick()` expose progress.
- Stable `PlayerId` seats use zero-based indices.

Frontends own input and presentation; NPCs choose actions from observations.
A future backend must map authenticated users to their seat before returning
observations. Do not expose the authoritative `Game` or its debug output to clients.
The existing `player` module remains available for compatibility; the new engine
does not use its hand-owning player types or terminal input helpers.

Actions, events, and observations are plain typed data, with no UI or transport
dependencies. Serialization and persistent recording are deferred to phase 5.

## Development checks

Build and run the tests locally:

```fish
cargo build
cargo test
```

If you contribute, please file issues or PRs against the repository:
https://github.com/Vale54321/schafkopf-logic

## License

This crate is licensed under the MIT license — see `LICENSE-MIT` for details.

## Maintainer

Valentin Heiserer <valentin@heiserer.de>

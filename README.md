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
let events = game.apply(player, observation.legal_actions[0])?;
# Ok::<(), schafkopf_logic::game::GameError>(())
```

- `observation(player)` returns a detached snapshot containing that player's
  hand, public hand sizes, turn, played cards, and eventual trick winner.
- `legal_actions(player)` returns legal actions for the acting player, or an
  empty list for other seats and completed tricks.
- Observations include these legal actions; `legal_cards()` iterates their cards
  in hand order for frontends and NPCs.
- `apply(player, action)` validates before changing state and returns public
  events. Errors are structured `GameError` values. No mutation occurs on error.
- Invalid moves return `MustFollowSuit` or `MustFollowTrump` without changing
  hands, turns, or public plays.
- `next_player()`, `phase()`, and `completed_trick()` expose progress.
- Stable `PlayerId` seats use zero-based indices.

Frontends own input and presentation; NPCs choose actions from observations.
A future backend must map authenticated users to their seat before returning
observations. Do not expose the authoritative `Game` or its debug output to clients.
The existing `player` module remains available for compatibility; the new engine
does not use its hand-owning player types or terminal input helpers.

## Following-suit rules and current limits

Trump is a separate category from a card's printed suit. Players must follow the
led category when they can; otherwise they may play any card. There is no
requirement to beat the current winning card or to trump when void in a suit.
Classification is shared with trick evaluation for Solo, Wenz, Geier, their suit
variants, Sauspiel, Bettel, and Ramsch.

These ordinary following rules use sections 2, 5.1.2, and 8.2 of the
[Schafkopfschule rules](https://schafkopfschule.de/regeln.html?file=files%2Finhalte%2Fdokumente%2FSpielen%2FRegeln%2FSchafkopfregeln-Aktuell-2024-04.pdf).
Sauspiel's called-ace restrictions and partnerships still require later match
state; only its ordinary following-suit rules are enforced here.

Bettel uses the chosen house variant: the same 32-card deck, trumps, trick
ordering, and following rules as Herz Solo. All Ober, then all Unter, then
hearts are trump. Other suited cards rank A > 10 > K > 9 > 8 > 7.
Players must follow the led suit/trump when possible, without compulsory
overtaking. The soloist's goal remains to take no tricks, so taking a trick is
unfavorable. Full-match results need the later eight-trick match state; a
single-trick result does not declare a match win.

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

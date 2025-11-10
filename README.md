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

**Crate:** `schafkopf-logic` • **Version:** 0.1.0

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

    // form a sample trick from the first card of each hand
    let trick = [&hands[0][0], &hands[1][0], &hands[2][0], &hands[3][0]];
    let winner = Gamemode::Sauspiel(Suit::Herz).winning_card(trick);
    println!("Winning card: {}", winner);

    // rank points example
    assert_eq!(Rank::Ass.points(), 11);
}
```

## Development

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

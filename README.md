# schafkopf-logic

A frontend-independent Schafkopf engine with complete eight-trick games,
declarations, legal actions, scoring, NPC policies, and replayable recordings.


**Crate:** [`schafkopf-logic`](https://crates.io/crates/schafkopf-logic)

## Features

- Deck and card types (suits, ranks, cards) with Display implementations
- Shuffling and dealing (4 players × 8 cards)
- Rank point values and helpers
- Game mode logic to determine trick winners (Sauspiel, Solo, Wenz, Geier,
  Bettel, Ramsch)

## Quick example

```rust
use schafkopf_logic::deck::{Deck, Suit};
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
        declarer: player,
    },
    Deck::new(),
)?;
let observation = game.observation(player);
let events = game.apply(player, observation.legal_actions[0])?;
// Continue submitting the current player's legal actions until game.result() is Some.
# Ok::<(), schafkopf_logic::game::GameError>(())
```

- `observation(player)` returns a detached snapshot containing that player's
  hand, public hand sizes, turn, played cards, and eventual trick winner.
- `legal_actions(player)` returns legal actions for the acting player, or an
  empty list for other seats and finished matches.
- Observations include these legal actions; `legal_cards()` iterates their cards
  in hand order for frontends and NPCs.
- `apply(player, action)` validates before changing state and returns public
  events. Errors are structured `GameError` values. No mutation occurs on error.
- Invalid moves return `MustFollowSuit` or `MustFollowTrump` without changing
  hands, turns, or public plays.
- `next_player()`, `phase()`, and `completed_trick()` expose progress.
- `completed_tricks()` preserves all eight tricks, in order. `result()` is available
  only after the final trick; `GameFinished` is emitted once.
- Observations include public points, tricks taken, completed trick count, and
  the final result. A completed trick remains visible until the next leader plays.
- The previous trick's winner leads next. There is no extra advance action or
  timer in the engine; presentation delays belong to the frontend.
- Stable `PlayerId` seats use zero-based indices.

Frontends own input and presentation; NPCs choose actions from observations.
A future backend must map authenticated users to their seat before returning
observations. Do not expose the authoritative `Game` or its debug output to clients.
The existing `player` module remains available for compatibility; the new engine
does not use its hand-owning player types or terminal input helpers.

## Match rules and scoring

Trump is a separate category from a card's printed suit. Players must follow the
led category when they can; otherwise they may play any card. There is no
requirement to beat the current winning card or to trump when void in a suit.
Classification is shared with trick evaluation for Solo, Wenz, Geier, their suit
variants, Sauspiel, Bettel, and Ramsch.

`Gamemode::sort_hand(&mut cards)` orders a displayed hand with trumps first,
strongest to weakest, followed by non-trump suits in Eichel/Gras/Herz/Schell
order and descending rank within each suit. It uses the same trump classification
and strength functions as trick evaluation. Sort an observation copy to avoid
changing the engine's hand order.

These ordinary following rules use sections 2, 5.1.2, and 8.2 of the
[Schafkopfschule rules](https://schafkopfschule.de/regeln.html?file=files%2Finhalte%2Fdokumente%2FSpielen%2FRegeln%2FSchafkopfregeln-Aktuell-2024-04.pdf).
Sauspiel validates the called suit and declarer's hand, locates the partner,
forces the called ace when its suit is led, restricts early discards of the ace,
and supports leading below it with at least four called-suit cards (Davonlaufen).
The partner is hidden from other players until the ace is played. Its holder
can see their own partnership. The last-trick exception is supported.

Bettel uses the chosen house variant: the same 32-card deck, trumps, trick
ordering, and following rules as Herz Solo. All Ober, then all Unter, then
hearts are trump. Other suited cards rank A > 10 > K > 9 > 8 > 7.
Players must follow the led suit/trump when possible, without compulsory
overtaking. The soloist's goal remains to take no tricks, so taking a trick is
unfavorable. The declarer wins only when taking zero tricks across all eight.

The deck totals 120 points (Unter is worth 2). Solo, Wenz, Geier, their suit
variants, and Sauspiel require at least 61 points for the declaring side.
The three defenders win a Solo at a 60-60 split. Sauspiel combines the declarer's
and partner's points before evaluating the threshold.

Ramsch uses the selected house rules: all players tied for most points lose;
a player taking all eight tricks wins by Durchmarsch. Points and trick counts
are returned with structured result reasons for every mode.

`GameConfig::declarer` is independent of `first_player`. Payment/tariff calculations and match series are outside the engine's current scope.

Actions, events, and observations are plain typed data, with no UI or transport
dependencies. Serializable actions, events, and versioned round records support replay and frontend-owned persistence.

## Declaration round and hand ratings

`round::Round` wraps `Game` with a frontend-independent declaration phase. Give it
an already shuffled full deck and a first player. Each seat acts once, in order,
using `declare(player, Some(mode))` to announce or `declare(player, None)` to pass.
A later declaration must have strictly higher priority. Equal ranks belong to
the earlier seat. Four passes select Ramsch. The cards do not change when the
winning contract and declarer are selected. The configured first player leads,
regardless of who declares. Use `play(player, action)` after declarations finish.

The current house order is Solo > pure Wenz/Geier > suit Wenz/Geier > Bettel >
Sauspiel. Wenz and Geier share a priority, as do their suit variants; suits have
no priority. This is a single declaration round, not a repeated tournament
auction. Ramsch is a fallback and cannot be announced.

`rating::rate_hand` and `ratings` use only the supplied hand. Scores range from
0 to 100 and estimate suitability, not win probability. Point games reward
strong trumps, side aces, protected tens, and trump length. Sauspiel gets a
partner bonus, but illegal calls are marked ineligible. House Bettel starts at
100 and subtracts risk for Ober, Unter, hearts, aces, and other high cards;
low non-trumps make good Bettel hands. `npc_declaration` picks the best eligible
higher-ranked game at 65 or above (80 for Bettel), otherwise passing.

`Round::observation(player)` hides legal card actions until declarations finish.
`next_bidder`, `legal_declarations`, `leading_bid`, and `declarations` supply the
announcement UI. As with `Game`, bind player IDs to authenticated seats in a
future backend. Player ratings must be requested for that same seat.

## Serializable game records

`Round::record()` returns a versioned `RoundRecord` containing rule identifiers,
rating version, exact deal order, initial hands, per-mode ratings, declarations,
final config, accepted actions with their emitted events, and final result.
It includes **all private hands** and belongs in trusted storage, never in a
player-facing live response. Serde serialization is provided; writing files,
timestamps, IDs, and storage policy belong to the frontend.

To reproduce a round, build `Deck::from_deal_order(record.deal_order.clone())`,
create a new `Round` with `record.first_player`, submit the recorded declarations,
then submit each recorded play. Invalid or repeated cards are rejected by the
deck constructor. The JSON round-trip/replay test verifies identical events and
results for a complete match. `PlayerId` deserialization rejects seats outside
0..4. Normal round records use schema 2, forced-contract records use schema 3,
and hand ratings use version 1. Use the supplied replay helpers to handle normal
and forced journals correctly.

## NPC strategies

`npc::NpcStrategy` is the replaceable policy boundary. A policy gets its own
`Observation` for card play and `DeclarationObservation` for bidding; it never
receives the authoritative game, other hands, or full journals. `choose_action`
returns an optional `Decision` (action and a typed reason). `choose_declaration`
returns a game or passes. The engine validates policy outputs as usual. Call
`reset()` when reusing a policy for another game; the self-play runner does this.

`HeuristicNpc` version 1 is deterministic and shares the existing hand-rating
bidding policy. Card tactics estimate trick control using unseen cards and
publicly proven voids. They conserve expensive winners, feed points to known
teammates, preserve points when losing, search the called ace, and draw trump
from strong declaring hands. Sauspiel teams use only information available to
the observer. Bettel declarers avoid tricks and shed dangerous losing cards;
defenders try to force the declarer to win. Ramsch avoids points, discards them
on opponents, and handles late attempts to achieve or prevent all-eight-trick
Durchmarsch. These are coded heuristics with uniform unseen-card estimates,
not calibrated probabilities or expert play.

`Observation::history` contains all completed public tricks, so a policy can
reconstruct seen cards and void suits even when called only on its own turn.
A retained completed table is treated as a new lead. `Gamemode::winning_index`
evaluates a partial trick using the same ordering as full trick evaluation.

`FirstLegal` version 1 preserves the old card policy as a comparison baseline;
it deliberately uses the same declaration policy. `StrategyInfo` and
`RecordedDecision` provide strategy versions, exact private/public policy inputs,
legal actions, chosen actions, and decision reasons for data collection.

## Development checks

Build and run the tests locally:

```fish
cargo build
cargo test
```

If you contribute, please file issues or PRs against the repository:
https://git.heiserer.de/valentin/schafkopf-logic

## License

This crate is licensed under the MIT license — see `LICENSE-MIT` for details.

## Maintainer

Valentin Heiserer <valentin@heiserer.de>

## Forced contracts

`Round::force_game(player, mode)` lets the current bidder start a hand-legal
contract immediately, overriding bidding priority. This is an explicit optional
house override; `declare` keeps its ordinary rules. It cannot change a running
game, force Ramsch, or bypass Sauspiel eligibility. The original deal, opening
seat, and previous declarations remain intact. Players who have not declared are
skipped, not recorded as passes. No card play is performed by forcing a contract.

Forced rounds use schema 3 and include `forced_contract: { player, mode }`.
Normal rounds continue using schema 2. For replay, submit the recorded ordinary
declarations, apply `force_game` when `forced_contract` is present, then replay
card actions. `CompactRecord` exports forced games with format 2, validates the
override on replay, and incorporates it into the content ID. Normal compact
records remain format 1 with unchanged IDs. Full-dataset replay accepts forced
rounds too. Existing recordings without the optional field still decode.

The replay exporter supplies no bidding-training examples for forced games:
the override changes the outcome of prior bids. The original bidding journal is
preserved in `round`, and validated card-play samples remain available.

//! Deterministic, I/O-free eight-trick matches shared by all frontends.
//! The engine owns hands, validates actions, collects tricks, and scores results.

mod rules;
mod sauspiel;
mod scoring;
mod types;
pub use types::*;

use crate::{
    deck::{Card, Deck},
    gamemode::Gamemode,
    observation::Observation,
};
use sauspiel::Sauspiel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    config: GameConfig,
    hands: [Vec<Card>; 4],
    next_player: Option<PlayerId>,
    trick: Vec<PlayedCard>,
    tricks: Vec<CompletedTrick>,
    points: [u16; 4],
    tricks_won: [u8; 4],
    sauspiel: Option<Sauspiel>,
    result: Option<GameResult>,
}

impl Game {
    /// Takes a full deck. Shuffling belongs to the caller; equal initial decks
    /// and accepted actions reproduce the same complete match.
    pub fn new(config: GameConfig, mut deck: Deck) -> Result<Self, GameError> {
        let cards_remaining = deck.iter().count();
        let hands = deck
            .deal_4x8()
            .ok_or(GameError::InvalidDeck { cards_remaining })?;
        let sauspiel = match config.mode {
            Gamemode::Sauspiel(suit) => Some(Sauspiel::new(suit, config.declarer, &hands)?),
            _ => None,
        };
        Ok(Self {
            config,
            hands,
            sauspiel,
            next_player: Some(config.first_player),
            trick: Vec::with_capacity(4),
            tricks: Vec::with_capacity(8),
            points: [0; 4],
            tricks_won: [0; 4],
            result: None,
        })
    }

    pub fn phase(&self) -> GamePhase {
        if self.result.is_some() {
            GamePhase::Finished
        } else if self.trick.len() == 4 {
            GamePhase::TrickComplete
        } else {
            GamePhase::PlayingTrick
        }
    }

    pub fn next_player(&self) -> Option<PlayerId> {
        self.next_player
    }

    /// The current four-card trick, retained for presentation until the next play.
    pub fn completed_trick(&self) -> Option<CompletedTrick> {
        if self.trick.len() == 4 {
            self.tricks.last().copied()
        } else {
            None
        }
    }

    pub fn completed_tricks(&self) -> &[CompletedTrick] {
        &self.tricks
    }

    pub fn result(&self) -> Option<&GameResult> {
        self.result.as_ref()
    }

    fn lead(&self) -> Option<Card> {
        if self.trick.len() == 4 {
            None
        } else {
            self.trick.first().map(|play| play.card)
        }
    }

    /// A completed trick's winner can lead immediately. UI delays are outside
    /// the engine. Only finished matches and seats out of turn have no actions.
    pub fn legal_actions(&self, player: PlayerId) -> Vec<Action> {
        if self.next_player != Some(player) {
            return Vec::new();
        }
        let hand = &self.hands[player.index()];
        let lead = self.lead();
        let cards = rules::legal_cards(self.config.mode, hand, lead);
        let cards = if let Some(sauspiel) = &self.sauspiel {
            sauspiel.restrict(player, hand, lead, self.tricks.len() == 7, cards)
        } else {
            cards
        };
        cards.into_iter().map(Action::PlayCard).collect()
    }

    /// Contains the observer's private hand and public table information only.
    /// Backends must bind the observer ID to an authenticated seat.
    pub fn observation(&self, player: PlayerId) -> Observation {
        Observation {
            player,
            declarations: Vec::new(),
            config: self.config,
            hand: self.hands[player.index()].clone(),
            legal_actions: self.legal_actions(player),
            hand_sizes: std::array::from_fn(|index| self.hands[index].len()),
            next_player: self.next_player,
            trick: self.trick.clone(),
            winner: self.completed_trick().map(|trick| trick.winner),
            phase: self.phase(),
            points: self.points,
            tricks_won: self.tricks_won,
            completed_tricks: self.tricks.len() as u8,
            history: self.tricks.clone(),
            partner: self
                .sauspiel
                .as_ref()
                .and_then(|state| state.visible_partner(player)),
            result: self.result.clone(),
        }
    }

    /// Every validation happens before mutation. Failure preserves the full state.
    pub fn apply(&mut self, player: PlayerId, action: Action) -> Result<Vec<GameEvent>, GameError> {
        let expected = self.next_player.ok_or(GameError::GameAlreadyFinished)?;
        if player != expected {
            return Err(GameError::WrongTurn {
                expected,
                actual: player,
            });
        }
        let Action::PlayCard(card) = action;
        let hand = &self.hands[player.index()];
        let index = hand
            .iter()
            .position(|held| *held == card)
            .ok_or(GameError::CardNotInHand { player, card })?;
        let lead = self.lead();
        if !rules::legal_cards(self.config.mode, hand, lead).contains(&card) {
            let lead = lead.expect("all owned cards are ordinarily legal when leading");
            return Err(if self.config.mode.is_trump(&lead) {
                GameError::MustFollowTrump { player }
            } else {
                GameError::MustFollowSuit {
                    player,
                    suit: lead.suit,
                }
            });
        }
        if !self.legal_actions(player).contains(&action) {
            return Err(GameError::CalledAceRestriction { player });
        }

        if let Some(state) = &mut self.sauspiel {
            state.played(player, card, lead.is_none());
        }
        if self.trick.len() == 4 {
            self.trick.clear();
        }
        self.hands[player.index()].remove(index);
        let play = PlayedCard { player, card };
        self.trick.push(play);
        let mut events = vec![GameEvent::CardPlayed(play)];
        if self.trick.len() < 4 {
            self.next_player = Some(player.next());
            return Ok(events);
        }

        let plays: [PlayedCard; 4] = std::array::from_fn(|index| self.trick[index]);
        let cards = plays.map(|play| play.card);
        let winning_card = self.config.mode.winning_card(cards.each_ref());
        let winner = plays
            .iter()
            .find(|play| play.card == *winning_card)
            .expect("winning card belongs to the trick")
            .player;
        let completed = CompletedTrick { plays, winner };
        self.points[winner.index()] += completed.points();
        self.tricks_won[winner.index()] += 1;
        self.tricks.push(completed);
        events.push(GameEvent::TrickCompleted(completed));
        if self.tricks.len() == 8 {
            debug_assert_eq!(self.points.iter().sum::<u16>(), 120);
            let result = scoring::result(
                self.config,
                self.sauspiel.as_ref().map(|state| state.partner),
                self.points,
                self.tricks_won,
            );
            self.result = Some(result.clone());
            self.next_player = None;
            events.push(GameEvent::GameFinished(result));
        } else {
            self.next_player = Some(winner);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests;

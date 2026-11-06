//! Deterministic, I/O-free game transitions shared by terminal, native, and web clients.
//!
//! This phase implements exactly one trick. Turn order and card ownership are
//! enforced, but following suit/trump is deliberately deferred to phase 3.
//! All modes use the existing trick evaluator; this is not complete match logic.

mod types;
pub use types::*;

use crate::{
    deck::{Card, Deck},
    observation::Observation,
};

/// Authoritative state. Frontends and NPCs should consume observations, not hands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    config: GameConfig,
    hands: [Vec<Card>; 4],
    next_player: Option<PlayerId>,
    trick: Vec<PlayedCard>,
    completed_trick: Option<CompletedTrick>,
}

impl Game {
    /// Takes ownership of a full deck. Identical decks and actions yield identical outcomes.
    /// Pass a shuffled deck for normal play, or Deck::new() for reproducible examples.
    pub fn new(config: GameConfig, mut deck: Deck) -> Result<Self, GameError> {
        let cards_remaining = deck.iter().count();
        let hands = deck
            .deal_4x8()
            .ok_or(GameError::InvalidDeck { cards_remaining })?;
        Ok(Self {
            config,
            hands,
            next_player: Some(config.first_player),
            trick: Vec::with_capacity(4),
            completed_trick: None,
        })
    }

    pub fn phase(&self) -> GamePhase {
        if self.completed_trick.is_some() {
            GamePhase::TrickComplete
        } else {
            GamePhase::PlayingTrick
        }
    }

    pub fn next_player(&self) -> Option<PlayerId> {
        self.next_player
    }

    pub fn completed_trick(&self) -> Option<CompletedTrick> {
        self.completed_trick
    }

    /// Returns an owned snapshot with only this player's private cards.
    /// A backend must bind this ID to the authenticated player's seat.
    pub fn observation(&self, player: PlayerId) -> Observation {
        Observation {
            player,
            config: self.config,
            hand: self.hands[player.index()].clone(),
            hand_sizes: std::array::from_fn(|index| self.hands[index].len()),
            next_player: self.next_player,
            trick: self.trick.clone(),
            winner: self.completed_trick.map(|trick| trick.winner),
        }
    }

    /// Validates an action before mutation. Rejected actions leave all state unchanged.
    /// Until phase 3, any card in the acting player's hand may be played.
    pub fn apply(&mut self, player: PlayerId, action: Action) -> Result<Vec<GameEvent>, GameError> {
        let expected = self.next_player.ok_or(GameError::TrickAlreadyComplete)?;
        if player != expected {
            return Err(GameError::WrongTurn {
                expected,
                actual: player,
            });
        }

        match action {
            Action::PlayCard(card) => {
                let hand = &mut self.hands[player.index()];
                let index = hand
                    .iter()
                    .position(|held| *held == card)
                    .ok_or(GameError::CardNotInHand { player, card })?;

                hand.remove(index);
                let play = PlayedCard { player, card };
                self.trick.push(play);
                let mut events = vec![GameEvent::CardPlayed(play)];

                if self.trick.len() == 4 {
                    let plays: [PlayedCard; 4] = std::array::from_fn(|index| self.trick[index]);
                    let cards = plays.map(|play| play.card);
                    let winning_card = self.config.mode.winning_card(cards.each_ref());
                    let winner = plays
                        .iter()
                        .find(|play| play.card == *winning_card)
                        .expect("the winning card belongs to this trick")
                        .player;
                    let completed = CompletedTrick { plays, winner };
                    self.completed_trick = Some(completed);
                    self.next_player = None;
                    events.push(GameEvent::TrickCompleted(completed));
                } else {
                    self.next_player = Some(player.next());
                }
                Ok(events)
            }
        }
    }
}

#[cfg(test)]
mod tests;

//! Player-facing snapshots for frontends and NPCs. Other hands stay in the engine.

use crate::{
    deck::Card,
    game::{Action, GameConfig, PlayedCard, PlayerId},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub player: PlayerId,
    pub config: GameConfig,
    pub hand: Vec<Card>,
    /// Empty unless this player may act. Supplied by the authoritative engine.
    pub legal_actions: Vec<Action>,
    /// Counts are public, indexed by PlayerId::index().
    pub hand_sizes: [usize; 4],
    pub next_player: Option<PlayerId>,
    /// Public plays in chronological order, including the completed trick.
    pub trick: Vec<PlayedCard>,
    pub winner: Option<PlayerId>,
}

impl Observation {
    /// Playable cards in hand order, suitable for a card UI or a simple NPC.
    pub fn legal_cards(&self) -> impl Iterator<Item = Card> + '_ {
        self.legal_actions
            .iter()
            .map(|Action::PlayCard(card)| *card)
    }
}

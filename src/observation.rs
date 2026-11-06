//! Player-facing snapshots for frontends and NPCs. Other hands stay in the engine.

use crate::{
    deck::Card,
    game::{GameConfig, PlayedCard, PlayerId},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub player: PlayerId,
    pub config: GameConfig,
    pub hand: Vec<Card>,
    /// Counts are public, indexed by PlayerId::index().
    pub hand_sizes: [usize; 4],
    pub next_player: Option<PlayerId>,
    /// Public plays in chronological order, including the completed trick.
    pub trick: Vec<PlayedCard>,
    pub winner: Option<PlayerId>,
}

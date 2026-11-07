//! Player-facing snapshots for frontends and NPCs. Other hands stay in the engine.

use crate::{
    deck::Card,
    game::{Action, CompletedTrick, GameConfig, GamePhase, GameResult, PlayedCard, PlayerId},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Observation {
    pub player: PlayerId,
    /// Public bidding history, populated by Round (empty for standalone Game).
    #[serde(default)]
    pub declarations: Vec<crate::round::Declaration>,
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
    pub phase: GamePhase,
    /// Points and trick counts already collected by each seat; always public.
    pub points: [u16; 4],
    pub tricks_won: [u8; 4],
    pub completed_tricks: u8,
    /// All completed tricks are public; policies can reconstruct card and suit memory.
    #[serde(default)]
    pub history: Vec<CompletedTrick>,
    /// Hidden until the called ace is played, except to its holder.
    pub partner: Option<PlayerId>,
    pub result: Option<GameResult>,
}

impl Observation {
    /// The first seat in the visible trick, or the configured opener before play.
    /// A completed trick remains visible until the next leader plays.
    pub fn trick_leader(&self) -> PlayerId {
        self.trick
            .first()
            .map_or(self.config.first_player, |play| play.player)
    }

    /// Who will open a trick when no trick is in progress. None during play or after the game.
    pub fn next_trick_leader(&self) -> Option<PlayerId> {
        match self.phase {
            GamePhase::TrickComplete => self.next_player,
            GamePhase::PlayingTrick if self.trick.is_empty() => self.next_player,
            _ => None,
        }
    }

    /// Playable cards in hand order, suitable for a card UI or a simple NPC.
    pub fn legal_cards(&self) -> impl Iterator<Item = Card> + '_ {
        self.legal_actions
            .iter()
            .map(|Action::PlayCard(card)| *card)
    }
}

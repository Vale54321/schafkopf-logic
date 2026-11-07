//! Portable trusted recordings for any frontend. No filesystem, RNG, or ML runtime.
//! Compact journals require this rules implementation to reconstruct observations.

use crate::{
    deck::{Card, Deck},
    game::{Action, GameResult, PlayerId},
    npc::{DecisionReason, StrategyInfo},
    observation::Observation,
    round::{Declaration, ForcedContract, RULES, RecordedDeclaration, Round, RoundRecord},
    self_play::DatasetGame,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Stable journal mode IDs, including pass=0 and all supported suit variants.
pub fn mode_id(mode: Option<crate::gamemode::Gamemode>) -> u8 {
    use crate::{deck::Suit, gamemode::Gamemode::*};
    let suit_id = |suit| match suit {
        Suit::Eichel => 0,
        Suit::Gras => 1,
        Suit::Herz => 2,
        Suit::Schell => 3,
    };
    match mode {
        None => 0,
        Some(Sauspiel(s)) => 1 + suit_id(s),
        Some(Solo(s)) => 5 + suit_id(s),
        Some(Wenz(None)) => 9,
        Some(Wenz(Some(s))) => 10 + suit_id(s),
        Some(Geier(None)) => 14,
        Some(Geier(Some(s))) => 15 + suit_id(s),
        Some(Bettel) => 19,
        Some(Ramsch) => 20,
    }
}

/// Same hands yield the same grouping ID regardless of hand order, seat
/// permutation, policy, leader, and rules. Use this for train/test grouping.
pub fn deal_id(deal_order: &[Card]) -> String {
    let mut hands: [Vec<u8>; 4] = std::array::from_fn(|seat| {
        let mut hand: Vec<_> = deal_order
            .iter()
            .skip(seat)
            .step_by(4)
            .map(|c| c.id())
            .collect();
        hand.sort_unstable();
        hand
    });
    hands.sort();
    digest(&hands.concat())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactRecord {
    pub format_version: u32,
    pub rules: String,
    pub rating_version: u32,
    pub first_player: PlayerId,
    /// Stable card IDs in chronological deal order.
    pub deal: Vec<u8>,
    pub declarations: Vec<Declaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced_contract: Option<ForcedContract>,
    /// (seat, stable card ID), in chronological play order.
    pub plays: Vec<(PlayerId, u8)>,
    pub result: GameResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactDatasetGame {
    pub dataset_version: u32,
    pub deal_index: u64,
    pub seed: u64,
    pub variant: String,
    pub deal_id: String,
    pub game_id: String,
    pub strategies: [StrategyInfo; 4],
    /// Optional teaching annotations; empty for human/unannotated recordings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<DecisionReason>,
    pub compact: CompactRecord,
}

#[derive(Debug)]
pub struct ReplayError(pub String);
impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for ReplayError {}
fn invalid(message: impl Into<String>) -> ReplayError {
    ReplayError(message.into())
}

impl CompactRecord {
    /// Export a completed human, NPC, server, or GUI round using its common journal.
    pub fn from_round(record: &RoundRecord) -> Result<Self, ReplayError> {
        if record.result.is_none()
            || record.plays.len() != 32
            || (record.forced_contract.is_none() && record.declarations.len() != 4)
            || (record.forced_contract.is_some() && record.declarations.len() >= 4)
        {
            return Err(invalid("only complete games can be archived"));
        }
        Ok(Self {
            format_version: if record.forced_contract.is_some() {
                2
            } else {
                1
            },
            rules: record.rules.clone(),
            rating_version: record.rating_version,
            first_player: record.first_player,
            deal: record.deal_order.iter().map(|c| c.id()).collect(),
            declarations: record.declarations.clone(),
            forced_contract: record.forced_contract,
            plays: record
                .plays
                .iter()
                .map(|p| {
                    let Action::PlayCard(card) = p.action;
                    (p.player, card.id())
                })
                .collect(),
            result: record.result.clone().unwrap(),
        })
    }

    pub fn cards(&self) -> Result<Vec<Card>, ReplayError> {
        self.deal
            .iter()
            .map(|id| Card::from_id(*id).ok_or_else(|| invalid("invalid card ID")))
            .collect()
    }

    /// Hash of the episode, independent of collection run and teaching policy.
    pub fn game_id(&self) -> Result<String, ReplayError> {
        // Explicit encoding; never relies on Rust's unstable DefaultHasher.
        let mut bytes = self.rules.as_bytes().to_vec();
        bytes.push(0);
        bytes.extend(self.rating_version.to_le_bytes());
        bytes.push(self.first_player.index() as u8);
        bytes.extend(&self.deal);
        for d in &self.declarations {
            bytes.push(d.player.index() as u8);
            bytes.push(mode_id(d.mode));
            bytes.push(0);
        }
        if let Some(forced) = self.forced_contract {
            // 255 is not a seat ID: normal journal encodings remain unchanged.
            bytes.extend([255, forced.player.index() as u8, mode_id(Some(forced.mode))]);
        }
        for (player, card) in &self.plays {
            bytes.extend([player.index() as u8, *card]);
        }
        Ok(digest(&bytes))
    }

    /// Reconstruct and validate every action and final result. Observations are
    /// returned only after validation, so corrupt records cannot emit samples.
    pub fn replay(&self) -> Result<ReplayedGame, ReplayError> {
        let supported = matches!(
            (self.format_version, self.forced_contract),
            (1, None) | (2, Some(_))
        );
        if !supported || self.rules != RULES || self.rating_version != 1 {
            return Err(invalid(
                "unsupported recording/rules/rating version; use its matching engine",
            ));
        }
        if self.plays.len() != 32
            || (self.forced_contract.is_none() && self.declarations.len() != 4)
            || (self.forced_contract.is_some() && self.declarations.len() >= 4)
        {
            return Err(invalid("invalid declaration/override count or play count"));
        }
        let deck = Deck::from_deal_order(self.cards()?).map_err(invalid)?;
        let mut round = Round::new(deck, self.first_player).map_err(|e| invalid(e.to_string()))?;
        for declaration in &self.declarations {
            round
                .declare(declaration.player, declaration.mode)
                .map_err(|e| invalid(e.to_string()))?;
            if round.declarations().last() != Some(declaration) {
                return Err(invalid("declaration score mismatch"));
            }
        }
        if let Some(forced) = self.forced_contract {
            round
                .force_game(forced.player, forced.mode)
                .map_err(|e| invalid(e.to_string()))?;
        }
        let mut decisions = Vec::with_capacity(32);
        for (player, id) in &self.plays {
            let action = Action::PlayCard(
                Card::from_id(*id).ok_or_else(|| invalid("invalid played card ID"))?,
            );
            let observation = round.observation(*player);
            round
                .play(*player, action)
                .map_err(|e| invalid(e.to_string()))?;
            decisions.push(ReplayedDecision {
                observation,
                action,
            });
        }
        if round.record().result.as_ref() != Some(&self.result) {
            return Err(invalid("final result mismatch"));
        }
        Ok(ReplayedGame {
            declarations: if self.forced_contract.is_some() {
                Vec::new()
            } else {
                round.record().declaration_decisions.clone()
            },
            decisions,
            round: round.record().clone(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ReplayedDecision {
    pub observation: Observation,
    pub action: Action,
}
#[derive(Debug, Serialize)]
pub struct ReplayedGame {
    /// Normal bidding examples only. An override changes the outcome of prior
    /// bids, so forced games supply card-play samples but no bidding samples.
    pub declarations: Vec<RecordedDeclaration>,
    pub decisions: Vec<ReplayedDecision>,
    pub round: RoundRecord,
}

impl CompactDatasetGame {
    pub fn from_full(game: &DatasetGame) -> Result<Self, ReplayError> {
        let compact = CompactRecord::from_round(&game.game.round)?;
        Ok(Self {
            dataset_version: 2,
            deal_index: game.deal_index,
            seed: game.seed,
            variant: game.variant.clone(),
            deal_id: deal_id(&game.game.round.deal_order),
            game_id: compact.game_id()?,
            strategies: game.game.strategies.clone(),
            reasons: game
                .game
                .decisions
                .iter()
                .map(|d| d.decision.reason)
                .collect(),
            compact,
        })
    }

    pub fn replay(&self) -> Result<ReplayedGame, ReplayError> {
        if self.dataset_version != 2
            || self.deal_id != deal_id(&self.compact.cards()?)
            || self.game_id != self.compact.game_id()?
        {
            return Err(invalid("dataset version or content ID mismatch"));
        }
        if !self.reasons.is_empty() && self.reasons.len() != 32 {
            return Err(invalid("invalid reason count"));
        }
        self.compact.replay()
    }
}

/// Validate older full recordings too, including every stored observation/event.
/// Legacy v1 recordings lacked declaration observations and public bid history.
pub fn replay_full(game: &DatasetGame) -> Result<ReplayedGame, ReplayError> {
    if game.dataset_version != 1 || ![1, 2, 3].contains(&game.game.round.schema_version) {
        return Err(invalid("unsupported full dataset/round version"));
    }
    let replay = CompactRecord::from_round(&game.game.round)?.replay()?;
    if game.game.decisions.len() != 32 {
        return Err(invalid("expected 32 decisions"));
    }
    for (turn, (stored, actual)) in game
        .game
        .decisions
        .iter()
        .zip(&replay.decisions)
        .enumerate()
    {
        let mut expected = actual.observation.clone();
        if game.game.round.schema_version == 1 {
            expected.declarations.clear();
        }
        if stored.turn as usize != turn
            || stored.observation != expected
            || stored.decision.action != actual.action
            || stored.strategy != game.game.strategies[expected.player.index()]
        {
            return Err(invalid(format!("decision {turn} mismatch")));
        }
    }
    let mut expected = replay.round.clone();
    if game.game.round.schema_version == 1 {
        expected.schema_version = 1;
        expected.declaration_decisions.clear();
    }
    if expected != game.game.round {
        return Err(invalid("round journal mismatch"));
    }
    Ok(replay)
}

#[cfg(test)]
mod tests;

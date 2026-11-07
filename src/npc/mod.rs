//! Replaceable policies. Policies receive seat observations, never Game/Round or
//! full records. The authoritative engine still validates every returned action.

mod heuristic;
mod knowledge;
pub use heuristic::HeuristicNpc;

use crate::{
    game::Action, gamemode::Gamemode, observation::Observation, rating,
    round::DeclarationObservation,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyInfo {
    pub name: String,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    ModelPolicy,
    FirstLegal,
    LeadTrump,
    LeadSideAce,
    SearchCalledAce,
    WinTrick,
    SupportTeammate,
    ConserveCards,
    AvoidBettelTrick,
    ForceBettelTrick,
    AvoidRamschPoints,
    ChaseDurchmarsch,
    StopDurchmarsch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub action: Action,
    pub reason: DecisionReason,
}

/// Exact policy input and chosen action for offline evaluation/training.
/// Contains only this seat's hand and public information at that moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedDecision {
    pub turn: u8,
    pub strategy: StrategyInfo,
    pub observation: Observation,
    pub decision: Decision,
}

pub trait NpcStrategy: Send + Sync {
    fn info(&self) -> StrategyInfo;
    /// Called before each new game, including when a policy instance is reused.
    fn reset(&mut self) {}
    fn choose_declaration(&mut self, view: &DeclarationObservation) -> Option<Gamemode> {
        if view.next_bidder != Some(view.player) {
            return None;
        }
        rating::npc_declaration(
            &view.hand,
            view.leading_bid.as_ref().and_then(|bid| bid.mode),
        )
        .filter(|mode| view.legal_modes.contains(mode))
    }
    /// None means no action is available (out of turn or finished).
    fn choose_action(&mut self, view: &Observation) -> Option<Decision>;
}

/// Previous behavior, retained as an explicit comparison baseline.
#[derive(Default)]
pub struct FirstLegal;

impl NpcStrategy for FirstLegal {
    fn info(&self) -> StrategyInfo {
        StrategyInfo {
            name: "first_legal".into(),
            version: 1,
        }
    }
    fn choose_action(&mut self, view: &Observation) -> Option<Decision> {
        if view.next_player != Some(view.player) {
            return None;
        }
        view.legal_actions.first().copied().map(|action| Decision {
            action,
            reason: DecisionReason::FirstLegal,
        })
    }
}

#[cfg(test)]
mod tests;

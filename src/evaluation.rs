//! Versioned evaluation data and rule-aware reconstruction, without ML or I/O.
//! Evaluation estimates are separate from authoritative game outcomes.
use crate::{
    agent::PolicyIdentity,
    deck::{Card, Deck},
    game::{Action, Game, GameConfig, GameResult, PlayerId, ResultReason},
    gamemode::Gamemode,
    observation::Observation,
    rating,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const EVALUATION_VERSION: u32 = 1;
pub const UTILITY_VERSION: &str = "balanced_win_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerOutcome {
    pub mode: Gamemode,
    pub role: String,
    pub won: bool,
    pub utility: f64,
    pub points: u16,
    pub tricks_won: u8,
    pub team_points: Option<u16>,
}

/// Artificial zero-sum benchmark utility, NOT a Schafkopf tariff/payout.
/// Winners share +1, losers share -1. No-winner/all-winner ties are neutral.
pub fn outcome(config: GameConfig, result: &GameResult, player: PlayerId) -> PlayerOutcome {
    let winners = PlayerId::ALL.iter().filter(|p| result.won(**p)).count();
    let won = result.won(player);
    let utility = if winners == 0 || winners == 4 {
        0.0
    } else if won {
        1.0 / winners as f64
    } else {
        -1.0 / (4 - winners) as f64
    };
    let partner = match result.reason {
        ResultReason::Points { partner, .. } => partner,
        _ => None,
    };
    let role = if config.mode == Gamemode::Ramsch {
        "individual"
    } else if player == config.declarer {
        "declarer"
    } else if Some(player) == partner {
        "partner"
    } else {
        "defender"
    };
    let team_points = match result.reason {
        ResultReason::Points { team_points, .. } => {
            Some(if player == config.declarer || Some(player) == partner {
                team_points
            } else {
                120 - team_points
            })
        }
        _ => None,
    };
    PlayerOutcome {
        mode: config.mode,
        role: role.into(),
        won,
        utility,
        points: result.points[player.index()],
        tricks_won: result.tricks_won[player.index()],
        team_points,
    }
}

/// Restore a hypothetical state from an initial deal and a player's public
/// transcript. Rejects hands inconsistent with ownership, follow rules, called
/// ace restrictions, public partner information, and the observed private hand.
pub fn reconstruct(view: &Observation, hands: &[Vec<Card>; 4]) -> Result<Game, String> {
    if hands.iter().any(|h| h.len() != 8)
        || view.next_player != Some(view.player)
        || view.result.is_some()
    {
        return Err(
            "expected eight initial cards per seat and an acting-player observation".into(),
        );
    }
    for bid in &view.declarations {
        if bid
            .mode
            .is_some_and(|mode| !rating::eligible(&hands[bid.player.index()], mode))
        {
            return Err("sample contradicts a declaration's eligibility".into());
        }
    }
    let order = (0..8)
        .flat_map(|i| hands.iter().map(move |hand| hand[i]))
        .collect();
    let mut game = Game::new(
        view.config,
        Deck::from_deal_order(order).map_err(str::to_owned)?,
    )
    .map_err(|e| e.to_string())?;
    for play in public_plays(view) {
        game.apply(play.player, Action::PlayCard(play.card))
            .map_err(|e| e.to_string())?;
    }
    let mut restored = game.observation(view.player);
    restored.declarations = view.declarations.clone();
    if restored != *view {
        return Err("sample does not reproduce the player's observation".into());
    }
    Ok(game)
}

pub fn public_plays(view: &Observation) -> Vec<crate::game::PlayedCard> {
    let mut plays: Vec<_> = view.history.iter().flat_map(|t| t.plays).collect();
    if view.trick.len() < 4 {
        plays.extend(&view.trick);
    }
    plays
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedOutcome {
    pub deal_id: String,
    pub seed: u64,
    pub deal_index: u64,
    pub seat: PlayerId,
    pub opponent: String,
    pub baseline: PlayerOutcome,
    pub candidate: PlayerOutcome,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionAssessment {
    pub action: Action,
    pub samples: u64,
    pub win_probability: f64,
    pub win_confidence95: Interval,
    pub mean_utility: f64,
    pub utility_confidence95: Interval,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionAssessment {
    pub schema_version: u32,
    pub rules: String,
    pub evaluator: String,
    pub knowledge: String,
    pub sampling: String,
    pub seed: u64,
    pub samples: u64,
    pub proposals: u64,
    pub discovery_samples: u64,
    pub player: PlayerId,
    pub mode: Gamemode,
    pub turn: u8,
    pub observation_id: String,
    pub chosen_action: Action,
    pub reference_action: Action,
    pub estimated_best_action: Action,
    pub actions: Vec<ActionAssessment>,
    pub estimated_decision_loss: f64,
    pub best_minus_chosen_confidence95: Interval,
    pub gain_over_reference: f64,
    pub gain_confidence95: Interval,
    pub assessment: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArenaConfig {
    pub scope: String,
    pub seed: u64,
    pub deals: u64,
    pub baseline: String,
    pub opponents: Vec<String>,
    pub batch_deals: u64,
    pub utility: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InferenceStats {
    pub decisions: u64,
    pub batches: u64,
    pub total_seconds: f64,
    pub max_batch_seconds: f64,
    pub illegal_responses: u64,
    pub timeouts: u64,
    pub protocol_errors: u64,
    pub transport_errors: u64,
    pub fallbacks: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Aggregate {
    pub pairs: u64,
    pub deal_clusters: u64,
    pub candidate_wins: u64,
    pub baseline_wins: u64,
    pub improved: u64,
    pub regressed: u64,
    pub mean_utility_delta: Option<f64>,
    pub confidence95: Option<Interval>,
    pub candidate_win_rate: Option<f64>,
    pub baseline_win_rate: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArenaReport {
    pub schema_version: u32,
    pub complete: bool,
    pub rules: String,
    pub config: ArenaConfig,
    pub candidate: PolicyIdentity,
    pub benchmark_id: String,
    pub metadata: BTreeMap<String, String>,
    pub overall: Aggregate,
    pub by_mode_role: BTreeMap<String, Aggregate>,
    pub by_opponent: BTreeMap<String, Aggregate>,
    pub inference: InferenceStats,
    pub failure: Option<String>,
}

#[cfg(test)]
mod tests;

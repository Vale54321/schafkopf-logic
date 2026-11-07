//! Headless simulations and paired policy comparisons. No filesystem or Bevy.

use crate::{
    deck::Deck,
    game::PlayerId,
    npc::{FirstLegal, HeuristicNpc, NpcStrategy, RecordedDecision, StrategyInfo},
    round::{Round, RoundError, RoundRecord},
};
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayedRound {
    pub strategies: [StrategyInfo; 4],
    pub decisions: Vec<RecordedDecision>,
    pub round: RoundRecord,
}

/// One complete game per JSONL row. Exact deal order is authoritative for
/// replay; the seed reproduces shuffling within the same RNG/library version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetGame {
    pub dataset_version: u32,
    pub deal_index: u64,
    pub seed: u64,
    pub variant: String,
    #[serde(flatten)]
    pub game: PlayedRound,
}

#[derive(Debug)]
pub enum SimulationError {
    Rules(RoundError),
    NoAction(PlayerId),
}
impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rules(error) => error.fmt(f),
            Self::NoAction(player) => write!(f, "{} returned no action on its turn", player),
        }
    }
}
impl std::error::Error for SimulationError {}
impl From<RoundError> for SimulationError {
    fn from(error: RoundError) -> Self {
        Self::Rules(error)
    }
}

pub fn heuristic_table() -> [Box<dyn NpcStrategy>; 4] {
    std::array::from_fn(|_| Box::new(HeuristicNpc) as Box<dyn NpcStrategy>)
}

pub fn seeded_deck(seed: u64) -> Deck {
    let mut cards = Deck::new().deal_order();
    cards.shuffle(&mut StdRng::seed_from_u64(seed));
    Deck::from_deal_order(cards).expect("shuffling preserves a complete deck")
}

/// Every policy sees only its own authenticated observation. Invalid policy
/// outputs fail instead of silently replacing a decision in training data.
pub fn play_round(
    deck: Deck,
    first: PlayerId,
    policies: &mut [Box<dyn NpcStrategy>; 4],
) -> Result<PlayedRound, SimulationError> {
    for policy in policies.iter_mut() {
        policy.reset();
    }
    let strategies = std::array::from_fn(|i| policies[i].info());
    let mut round = Round::new(deck, first).map_err(RoundError::from)?;
    while let Some(player) = round.next_bidder() {
        let choice =
            policies[player.index()].choose_declaration(&round.declaration_observation(player));
        round.declare(player, choice)?;
    }
    let mut decisions = Vec::with_capacity(32);
    while let Some(player) = round.observation(first).next_player {
        let observation = round.observation(player);
        let decision = policies[player.index()]
            .choose_action(&observation)
            .ok_or(SimulationError::NoAction(player))?;
        round.play(player, decision.action)?;
        decisions.push(RecordedDecision {
            turn: decisions.len() as u8,
            strategy: strategies[player.index()].clone(),
            observation,
            decision,
        });
    }
    Ok(PlayedRound {
        strategies,
        decisions,
        round: round.record().clone(),
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparisonStats {
    pub comparisons: u64,
    pub baseline_wins: u64,
    pub heuristic_wins: u64,
    pub improved: u64,
    pub regressed: u64,
    pub unchanged: u64,
}
impl ComparisonStats {
    fn merge(&mut self, other: Self) {
        self.comparisons += other.comparisons;
        self.baseline_wins += other.baseline_wins;
        self.heuristic_wins += other.heuristic_wins;
        self.improved += other.improved;
        self.regressed += other.regressed;
        self.unchanged += other.unchanged;
    }

    fn add(&mut self, baseline: bool, heuristic: bool) {
        self.comparisons += 1;
        self.baseline_wins += u64::from(baseline);
        self.heuristic_wins += u64::from(heuristic);
        match (baseline, heuristic) {
            (false, true) => self.improved += 1,
            (true, false) => self.regressed += 1,
            _ => self.unchanged += 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub deals: u64,
    pub overall: ComparisonStats,
    pub by_mode: BTreeMap<String, ComparisonStats>,
}

impl BenchmarkReport {
    /// Combine independently simulated deals without sharing mutable counters.
    pub fn merge(&mut self, other: Self) {
        self.deals += other.deals;
        self.overall.merge(other.overall);
        for (mode, stats) in other.by_mode {
            self.by_mode.entry(mode).or_default().merge(stats);
        }
    }
}

/// Identical deal, leader, declaration policy and contract for each comparison.
/// Compare each seat against its own all-first-legal baseline, replacing just
/// that seat's card policy. Five complete games are returned for one deal.
pub fn compare_deal(
    seed: u64,
    first: PlayerId,
    report: &mut BenchmarkReport,
) -> Result<Vec<PlayedRound>, SimulationError> {
    let deck = seeded_deck(seed);
    let mut baseline: [Box<dyn NpcStrategy>; 4] =
        std::array::from_fn(|_| Box::new(FirstLegal) as Box<dyn NpcStrategy>);
    let base = play_round(deck.clone(), first, &mut baseline)?;
    let config = base.round.config.unwrap();
    let mut games = vec![base];
    for player in PlayerId::ALL {
        let mut policies: [Box<dyn NpcStrategy>; 4] =
            std::array::from_fn(|_| Box::new(FirstLegal) as Box<dyn NpcStrategy>);
        policies[player.index()] = Box::new(HeuristicNpc);
        let game = play_round(deck.clone(), first, &mut policies)?;
        let base_won = games[0].round.result.as_ref().unwrap().won(player);
        let won = game.round.result.as_ref().unwrap().won(player);
        report.overall.add(base_won, won);
        report
            .by_mode
            .entry(format!("{:?}", config.mode))
            .or_default()
            .add(base_won, won);
        games.push(game);
    }
    report.deals += 1;
    Ok(games)
}

#[cfg(test)]
mod tests;

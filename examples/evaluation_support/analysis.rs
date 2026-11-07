use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use schafkopf_logic::{
    deck::{Card, Deck},
    evaluation::{self, ActionAssessment, DecisionAssessment, Interval},
    game::{Action, Game, PlayerId},
    npc::{FirstLegal, HeuristicNpc, NpcStrategy},
    observation::Observation,
    round::RULES,
};
use serde::Deserialize;
use std::collections::HashMap;

type Result<T> = std::result::Result<T, String>;
fn category(view: &Observation, card: Card) -> u8 {
    if view.config.mode.is_trump(&card) {
        4
    } else {
        card.id() / 8
    }
}
fn policy(name: &str) -> Result<Box<dyn NpcStrategy>> {
    match name {
        "heuristic" => Ok(Box::new(HeuristicNpc)),
        "first_legal" => Ok(Box::new(FirstLegal)),
        _ => Err("reference must be heuristic or first_legal".into()),
    }
}

/// Exact uniform assignment conditional on hand sizes and public voids. Dynamic
/// programming counts completions; greedy assignment would introduce bias.
struct Sampler {
    actor: PlayerId,
    seats: [usize; 3],
    pool: Vec<Card>,
    allowed: Vec<u8>,
    capacities: [u8; 3],
    fixed: [Vec<Card>; 4],
    memo: HashMap<(usize, [u8; 3]), u64>,
}
impl Sampler {
    fn new(view: &Observation) -> Result<Self> {
        if view.hand_sizes.iter().any(|n| *n > 8)
            || view.hand_sizes[view.player.index()] != view.hand.len()
        {
            return Err("invalid hand sizes".into());
        }
        let actor = view.player;
        let seats: Vec<_> = (0..4).filter(|s| *s != actor.index()).collect();
        let seats: [usize; 3] = seats.try_into().unwrap();
        let mut fixed: [Vec<Card>; 4] = Default::default();
        let mut voids = [0u8; 4];
        let plays = evaluation::public_plays(view);
        let mut trick = Vec::new();
        for play in &plays {
            if fixed.iter().any(|h| h.contains(&play.card)) || view.hand.contains(&play.card) {
                return Err("duplicate public/private card".into());
            }
            fixed[play.player.index()].push(play.card);
            if trick.is_empty() {
                trick.push(play.card);
            } else if category(view, play.card) != category(view, trick[0]) {
                voids[play.player.index()] |= 1 << category(view, trick[0]);
            }
            if fixed.iter().map(Vec::len).sum::<usize>() % 4 == 0 {
                trick.clear();
            }
        }
        fixed[actor.index()].extend(&view.hand);
        if fixed.iter().enumerate().any(|(s, h)| {
            h.len()
                + if s == actor.index() {
                    0
                } else {
                    view.hand_sizes[s]
                }
                != 8
        }) {
            return Err("hand sizes disagree with transcript".into());
        }
        let mut pool: Vec<_> = Deck::new()
            .iter()
            .copied()
            .filter(|c| !fixed.iter().any(|h| h.contains(c)))
            .collect();
        // Most constrained cards first reduces the number of DP states.
        pool.sort_by_key(|card| {
            seats
                .iter()
                .filter(|s| voids[**s] & (1 << category(view, *card)) == 0)
                .count()
        });
        let allowed = pool
            .iter()
            .map(|card| {
                seats.iter().enumerate().fold(0, |mask, (i, seat)| {
                    if voids[*seat] & (1 << category(view, *card)) == 0 {
                        mask | (1 << i)
                    } else {
                        mask
                    }
                })
            })
            .collect();
        let capacities = seats.map(|s| view.hand_sizes[s] as u8);
        let mut result = Self {
            actor,
            seats,
            pool,
            allowed,
            capacities,
            fixed,
            memo: HashMap::new(),
        };
        if result.count(0, capacities) == 0 {
            return Err("no hidden hands satisfy public voids".into());
        }
        Ok(result)
    }
    fn count(&mut self, index: usize, left: [u8; 3]) -> u64 {
        if index == self.pool.len() {
            return u64::from(left == [0; 3]);
        }
        if let Some(count) = self.memo.get(&(index, left)) {
            return *count;
        }
        let mut total = 0;
        for seat in 0..3 {
            if left[seat] > 0 && self.allowed[index] & (1 << seat) != 0 {
                let mut next = left;
                next[seat] -= 1;
                total += self.count(index + 1, next);
            }
        }
        self.memo.insert((index, left), total);
        total
    }
    fn draw(&mut self, view: &Observation, rng: &mut StdRng) -> Result<Game> {
        let mut left = self.capacities;
        let mut hands = self.fixed.clone();
        for index in 0..self.pool.len() {
            let mut draw = rng.random_range(0..self.count(index, left));
            for seat in 0..3 {
                if left[seat] == 0 || self.allowed[index] & (1 << seat) == 0 {
                    continue;
                }
                let mut next = left;
                next[seat] -= 1;
                let ways = self.count(index + 1, next);
                if draw < ways {
                    hands[self.seats[seat]].push(self.pool[index]);
                    left = next;
                    break;
                }
                draw -= ways;
            }
        }
        for (seat, hand) in hands.iter_mut().enumerate() {
            if seat == self.actor.index() {
                // Replay removes public cards, leaving the exact observed hand order.
                let public = self.fixed[seat].len() - view.hand.len();
                let played = hand[..public].to_vec();
                *hand = view.hand.clone();
                hand.extend(played);
            } else {
                hand.shuffle(rng);
            }
        }
        evaluation::reconstruct(view, &hands)
    }
}
fn rollout(
    mut game: Game,
    action: Action,
    view: &Observation,
    reference: &str,
) -> Result<(f64, bool)> {
    game.apply(view.player, action).map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn NpcStrategy>; 4] =
        std::array::from_fn(|_| policy(reference).unwrap());
    while let Some(player) = game.next_player() {
        let mut observed = game.observation(player);
        observed.declarations = view.declarations.clone();
        let action = if observed.legal_actions.len() == 1 {
            observed.legal_actions[0]
        } else {
            policies[player.index()]
                .choose_action(&observed)
                .ok_or("reference returned no action")?
                .action
        };
        game.apply(player, action).map_err(|e| e.to_string())?;
    }
    let result = evaluation::outcome(
        view.config,
        game.result().ok_or("rollout did not finish")?,
        view.player,
    );
    Ok((result.utility, result.won))
}
fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
fn bounded_ci(value: f64, n: usize, range: f64, low: f64, high: f64) -> Interval {
    let radius = range * ((2.0f64 / 0.05).ln() / (2.0 * n as f64)).sqrt();
    Interval {
        low: (value - radius).max(low),
        high: (value + radius).min(high),
    }
}
fn wilson(wins: usize, n: usize) -> Interval {
    let n = n as f64;
    let p = wins as f64 / n;
    let z2 = 1.96f64.powi(2);
    let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let radius = 1.96 * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
    Interval {
        low: (center - radius).max(0.0),
        high: (center + radius).min(1.0),
    }
}
#[derive(Deserialize)]
pub struct Analyze {
    pub observation: Observation,
    pub action: Action,
    pub samples: usize,
    pub seed: u64,
    pub reference: String,
}
pub fn analyze(input: Analyze) -> Result<DecisionAssessment> {
    if !(8..=100_000).contains(&input.samples)
        || !input.observation.legal_actions.contains(&input.action)
    {
        return Err("need 8..100000 samples and a legal chosen action".into());
    }
    let view = &input.observation;
    let reference_action = policy(&input.reference)?
        .choose_action(view)
        .ok_or("reference has no action")?
        .action;
    let actions = &view.legal_actions;
    let chosen = actions.iter().position(|a| *a == input.action).unwrap();
    let baseline = actions
        .iter()
        .position(|a| *a == reference_action)
        .ok_or("reference action is illegal")?;
    let mut sampler = Sampler::new(view)?;
    let mut rng = StdRng::seed_from_u64(input.seed);
    let mut utility = vec![Vec::new(); actions.len()];
    let mut wins = vec![Vec::new(); actions.len()];
    let mut proposals = 0u64;
    let max_proposals = input.samples.saturating_mul(1000).max(10_000);
    let mut accepted = 0;
    while accepted < input.samples && proposals < max_proposals as u64 {
        proposals += 1;
        let Ok(game) = sampler.draw(view, &mut rng) else {
            continue;
        };
        for (index, action) in actions.iter().enumerate() {
            let (value, win) = rollout(game.clone(), *action, view, &input.reference)?;
            utility[index].push(value);
            wins[index].push(win);
        }
        accepted += 1;
    }
    if accepted < input.samples {
        return Err(format!(
            "only {accepted}/{} valid hidden-hand samples after {proposals} proposals",
            input.samples
        ));
    }
    let split = input.samples / 2;
    let selected = (0..actions.len())
        .max_by(|a, b| mean(&utility[*a][..split]).total_cmp(&mean(&utility[*b][..split])))
        .unwrap();
    let evaluation_samples = input.samples - split;
    let compare = |a: usize, b: usize| {
        let differences: Vec<_> = utility[a][split..]
            .iter()
            .zip(&utility[b][split..])
            .map(|(a, b)| a - b)
            .collect();
        let delta = mean(&differences);
        let ci = if a == b {
            Interval {
                low: 0.0,
                high: 0.0,
            }
        } else {
            bounded_ci(delta, evaluation_samples, 4.0, -2.0, 2.0)
        };
        (delta, ci)
    };
    let (loss, loss_ci) = compare(selected, chosen);
    let (gain, gain_ci) = compare(chosen, baseline);
    let assessments = actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let m = mean(&utility[index][split..]);
            let victories = wins[index][split..].iter().filter(|w| **w).count();
            ActionAssessment {
                action: *action,
                samples: evaluation_samples as u64,
                mean_utility: m,
                utility_confidence95: bounded_ci(m, evaluation_samples, 2.0, -1.0, 1.0),
                win_probability: victories as f64 / evaluation_samples as f64,
                win_confidence95: wilson(victories, evaluation_samples),
            }
        })
        .collect();
    let assessment = if loss_ci.low > 0.02 {
        "estimated_avoidable_loss"
    } else if gain_ci.low > 0.02 {
        "above_reference"
    } else if chosen == selected {
        "no_detected_loss"
    } else {
        "inconclusive"
    };
    Ok(DecisionAssessment {
        schema_version: 1, rules: RULES.into(), evaluator: format!("uniform_hidden_hands+{}_v1", input.reference),
        knowledge: "player_information".into(), sampling: "uniform assignments conditioned on public legality; not bid-strength likelihoods".into(),
        seed: input.seed, samples: input.samples as u64, proposals, discovery_samples: split as u64,
        player: view.player, mode: view.config.mode, turn: evaluation::public_plays(view).len() as u8,
        observation_id: schafkopf_logic::recording::digest(&serde_json::to_vec(view).map_err(|e| e.to_string())?),
        chosen_action: input.action, reference_action, estimated_best_action: actions[selected], actions: assessments,
        estimated_decision_loss: loss.max(0.0), best_minus_chosen_confidence95: loss_ci,
        gain_over_reference: gain, gain_confidence95: gain_ci, assessment: assessment.into(),
        limitations: vec!["Reference-policy estimates, not optimal-play proofs or calibrated human win chances.".into(),
            "Confidence bounds concern sampling error, not errors in hidden-hand beliefs or continuation policies.".into(),
            "Best action selected on discovery samples; comparisons use separate evaluation samples.".into(),
            "A loss or unlikely win alone is not a decision-quality label.".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use schafkopf_logic::self_play;
    #[test]
    fn samples_reproduce_information_and_forced_moves_have_no_avoidable_loss() {
        let played = self_play::play_round(
            self_play::seeded_deck(42),
            PlayerId::ALL[0],
            &mut self_play::heuristic_table(),
        )
        .unwrap();
        let first = &played.decisions[0].observation;
        let mut initial = Sampler::new(first).unwrap();
        let capacities = initial.capacities;
        // Number of labeled assignments of 24 unseen cards into three 8-card hands.
        assert_eq!(initial.count(0, capacities), 9_465_511_770);
        for turn in [0, 9, 21, 31] {
            let view = &played.decisions[turn].observation;
            let mut sampler = Sampler::new(view).unwrap();
            let mut rng = StdRng::seed_from_u64(7);
            let mut valid = 0;
            for _ in 0..200 {
                if sampler.draw(view, &mut rng).is_ok() {
                    valid += 1;
                }
            }
            assert!(valid > 0, "turn {turn}");
        }
        let last = &played.decisions[31];
        let report = analyze(Analyze {
            observation: last.observation.clone(),
            action: last.decision.action,
            samples: 8,
            seed: 4,
            reference: "heuristic".into(),
        })
        .unwrap();
        assert_eq!(report.estimated_decision_loss, 0.0);
        assert_eq!(report.gain_over_reference, 0.0);
        assert_eq!(report.assessment, "no_detected_loss");
    }
}

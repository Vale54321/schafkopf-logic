use super::*;
use crate::{deck::Deck, self_play};
#[test]
fn benchmark_utility_is_zero_sum_and_replay_branching_preserves_observation() {
    for seed in 0..20 {
        let played = self_play::play_round(
            self_play::seeded_deck(seed),
            PlayerId::ALL[0],
            &mut self_play::heuristic_table(),
        )
        .unwrap();
        let config = played.round.config.unwrap();
        let result = played.round.result.as_ref().unwrap();
        let total: f64 = PlayerId::ALL
            .iter()
            .map(|p| outcome(config, result, *p).utility)
            .sum();
        assert!(total.abs() < 1e-12);
        for decision in &played.decisions {
            reconstruct(&decision.observation, &played.round.initial_hands).unwrap();
        }
    }
    let mut hands = Deck::new().deal_4x8().unwrap();
    hands[0].clear();
    let played = self_play::play_round(
        Deck::new(),
        PlayerId::ALL[0],
        &mut self_play::heuristic_table(),
    )
    .unwrap();
    assert!(reconstruct(&played.decisions[0].observation, &hands).is_err());
}

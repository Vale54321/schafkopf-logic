use super::*;
use crate::self_play::{self, DatasetGame};

fn sample() -> DatasetGame {
    DatasetGame {
        dataset_version: 1,
        deal_index: 0,
        seed: 42,
        variant: "test".into(),
        game: self_play::play_round(
            self_play::seeded_deck(42),
            PlayerId::ALL[0],
            &mut self_play::heuristic_table(),
        )
        .unwrap(),
    }
}

#[test]
fn compact_replay_recovers_all_policy_inputs_and_bidding_snapshots() {
    let full = sample();
    let compact = CompactDatasetGame::from_full(&full).unwrap();
    let decoded: CompactDatasetGame =
        serde_json::from_str(&serde_json::to_string(&compact).unwrap()).unwrap();
    let replay = decoded.replay().unwrap();
    assert_eq!(replay.round, full.game.round);
    assert_eq!(replay.declarations.len(), 4);
    for (actual, expected) in replay.decisions.iter().zip(&full.game.decisions) {
        assert_eq!(actual.observation, expected.observation);
        assert_eq!(actual.action, expected.decision.action);
        assert_eq!(actual.observation.declarations.len(), 4);
    }
    replay_full(&full).unwrap();
}

#[test]
fn tampering_versions_results_actions_ids_and_snapshots_is_rejected() {
    let full = sample();
    let compact = CompactDatasetGame::from_full(&full).unwrap();
    let mut changed = compact.clone();
    changed.compact.result.points[0] += 1;
    assert!(changed.replay().is_err());
    let mut changed = compact.clone();
    changed.compact.plays[0].1 = 255;
    assert!(changed.replay().is_err());
    let mut changed = compact.clone();
    changed.compact.rules = "unknown".into();
    assert!(changed.replay().is_err());
    let mut changed = compact;
    changed.deal_id = "wrong".into();
    assert!(changed.replay().is_err());
    let mut changed = full;
    changed.game.decisions[0].observation.hand.clear();
    assert!(replay_full(&changed).is_err());
}

#[test]
fn legacy_full_recordings_still_validate() {
    let mut full = sample();
    full.game.round.schema_version = 1;
    full.game.round.declaration_decisions.clear();
    for decision in &mut full.game.decisions {
        decision.observation.declarations.clear();
    }
    replay_full(&full).unwrap();
}

#[test]
fn deal_grouping_survives_seat_rotation_hand_order_and_different_policies() {
    let deal = self_play::seeded_deck(42).deal_order();
    let mut rotated = deal.clone();
    for chunk in rotated.chunks_mut(4) {
        chunk.rotate_left(1);
    }
    assert_eq!(deal_id(&deal), deal_id(&rotated));
    rotated.swap(0, 4);
    assert_eq!(deal_id(&deal), deal_id(&rotated));
    assert_ne!(
        deal_id(&deal),
        deal_id(&self_play::seeded_deck(43).deal_order())
    );
    for id in 0..32 {
        assert_eq!(Card::from_id(id).unwrap().id(), id);
    }
    assert!(Card::from_id(32).is_none());
}

#[test]
fn failed_bids_do_not_append_recording_snapshots() {
    let mut round = Round::new(Deck::new(), PlayerId::ALL[0]).unwrap();
    assert!(round.declare(PlayerId::ALL[1], None).is_err());
    assert!(round.record().declaration_decisions.is_empty());
}

#[test]
fn public_declarations_do_not_leak_private_hand_ratings() {
    let mut round = Round::new(Deck::new(), PlayerId::ALL[0]).unwrap();
    round
        .declare(PlayerId::ALL[0], Some(crate::gamemode::Gamemode::Bettel))
        .unwrap();
    assert!(round.record().declarations[0].score.is_some());
    let view = round.declaration_observation(PlayerId::ALL[1]);
    assert_eq!(view.leading_bid.unwrap().score, None);
    assert!(view.declarations.iter().all(|bid| bid.score.is_none()));
    assert!(
        round
            .observation(PlayerId::ALL[1])
            .declarations
            .iter()
            .all(|bid| bid.score.is_none())
    );
}

#[test]
fn forced_games_round_trip_and_replay_without_becoming_normal_bidding_examples() {
    use crate::{
        gamemode::Gamemode,
        round::{ForcedContract, Round, RoundPhase},
    };
    let mut round = Round::new(Deck::new(), PlayerId::ALL[0]).unwrap();
    round
        .force_game(PlayerId::ALL[0], Gamemode::Bettel)
        .unwrap();
    while round.phase() == RoundPhase::Playing {
        let player = round.observation(PlayerId::ALL[0]).next_player.unwrap();
        round
            .play(player, round.observation(player).legal_actions[0])
            .unwrap();
    }
    let compact = CompactRecord::from_round(round.record()).unwrap();
    assert_eq!(compact.format_version, 2);
    let json = serde_json::to_string(&compact).unwrap();
    let parsed: CompactRecord = serde_json::from_str(&json).unwrap();
    let replay = parsed.replay().unwrap();
    assert_eq!(replay.round, *round.record());
    assert_eq!(replay.decisions.len(), 32);
    assert!(replay.declarations.is_empty());
    assert!(replay.round.declarations.is_empty());
    let strategy = crate::npc::StrategyInfo {
        name: "first_legal".into(),
        version: 1,
    };
    let full = DatasetGame {
        dataset_version: 1,
        deal_index: 0,
        seed: 0,
        variant: "forced_test".into(),
        game: crate::self_play::PlayedRound {
            strategies: std::array::from_fn(|_| strategy.clone()),
            decisions: replay
                .decisions
                .iter()
                .enumerate()
                .map(|(turn, actual)| crate::npc::RecordedDecision {
                    turn: turn as u8,
                    strategy: strategy.clone(),
                    observation: actual.observation.clone(),
                    decision: crate::npc::Decision {
                        action: actual.action,
                        reason: DecisionReason::FirstLegal,
                    },
                })
                .collect(),
            round: round.record().clone(),
        },
    };
    assert_eq!(replay_full(&full).unwrap().round, *round.record());
    let mut changed = compact.clone();
    changed.forced_contract = Some(ForcedContract {
        player: PlayerId::ALL[1],
        mode: Gamemode::Bettel,
    });
    assert_ne!(compact.game_id().unwrap(), changed.game_id().unwrap());
    assert!(changed.replay().is_err());
    changed.forced_contract = None;
    assert!(changed.replay().is_err());
    changed = compact;
    changed.format_version = 1;
    assert!(changed.replay().is_err());
}

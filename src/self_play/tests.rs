use super::*;
use crate::{
    deck::{Card, Rank, Suit},
    game::{Action, Game, GameConfig},
    gamemode::Gamemode,
    npc::{Decision, NpcStrategy},
    observation::Observation,
    rating::DECLARABLE_MODES,
};

#[test]
fn seeded_self_play_is_reproducible_complete_and_replayable() {
    let first = play_round(seeded_deck(42), PlayerId::ALL[0], &mut heuristic_table()).unwrap();
    let second = play_round(seeded_deck(42), PlayerId::ALL[0], &mut heuristic_table()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.decisions.len(), 32);
    assert_eq!(first.round.plays.len(), 32);
    assert_eq!(
        first
            .round
            .result
            .as_ref()
            .unwrap()
            .points
            .iter()
            .sum::<u16>(),
        120
    );
    let json = serde_json::to_string(&first).unwrap();
    let parsed: PlayedRound = serde_json::from_str(&json).unwrap();
    let mut replay = Round::new(
        Deck::from_deal_order(parsed.round.deal_order.clone()).unwrap(),
        parsed.round.first_player,
    )
    .unwrap();
    for bid in &parsed.round.declarations {
        replay.declare(bid.player, bid.mode).unwrap();
    }
    for (turn, decision) in parsed.decisions.iter().enumerate() {
        assert_eq!(
            decision.observation,
            replay.observation(decision.observation.player)
        );
        assert!(
            decision
                .observation
                .legal_actions
                .contains(&decision.decision.action)
        );
        assert_eq!(decision.turn, turn as u8);
        assert_eq!(decision.strategy.name, "heuristic");
        replay
            .play(decision.observation.player, decision.decision.action)
            .unwrap();
    }
    assert_eq!(replay.record(), &parsed.round);
}

#[test]
fn tactics_are_legal_in_every_mode_from_every_opening_seat() {
    for mode in DECLARABLE_MODES.into_iter().chain([Gamemode::Ramsch]) {
        for first in PlayerId::ALL {
            let mut game = Game::new(
                GameConfig {
                    mode,
                    first_player: first,
                    declarer: PlayerId::ALL[0],
                },
                Deck::new(),
            )
            .unwrap();
            while let Some(player) = game.next_player() {
                let view = game.observation(player);
                let action = HeuristicNpc.choose_action(&view).unwrap().action;
                assert!(view.legal_actions.contains(&action), "{mode:?}");
                game.apply(player, action).unwrap();
            }
            assert_eq!(game.result().unwrap().points.iter().sum::<u16>(), 120);
        }
    }
}

#[test]
fn paired_benchmark_compares_all_seats_on_identical_deals_and_contracts() {
    let mut report = BenchmarkReport::default();
    let games = compare_deal(123, PlayerId::ALL[2], &mut report).unwrap();
    assert_eq!(games.len(), 5);
    assert_eq!(report.deals, 1);
    assert_eq!(report.overall.comparisons, 4);
    assert_eq!(
        report.overall.improved + report.overall.regressed + report.overall.unchanged,
        4
    );
    for game in &games[1..] {
        assert_eq!(game.round.deal_order, games[0].round.deal_order);
        assert_eq!(game.round.config, games[0].round.config);
        assert_eq!(game.round.declarations, games[0].round.declarations);
        assert_eq!(
            game.strategies
                .iter()
                .filter(|strategy| strategy.name == "heuristic")
                .count(),
            1
        );
    }
}

struct IllegalPolicy;
impl NpcStrategy for IllegalPolicy {
    fn info(&self) -> StrategyInfo {
        StrategyInfo {
            name: "illegal_test".into(),
            version: 1,
        }
    }
    fn choose_action(&mut self, view: &Observation) -> Option<Decision> {
        Some(Decision {
            action: Action::PlayCard(Card {
                suit: Suit::Eichel,
                rank: Rank::Ass,
            }),
            reason: crate::npc::DecisionReason::FirstLegal,
        })
        .filter(|_| view.next_player.is_some())
    }
}
#[test]
fn invalid_policy_outputs_are_reported_instead_of_silently_relabelled() {
    let mut table: [Box<dyn NpcStrategy>; 4] =
        std::array::from_fn(|_| Box::new(IllegalPolicy) as Box<dyn NpcStrategy>);
    assert!(play_round(Deck::new(), PlayerId::ALL[0], &mut table).is_err());
}

#[test]
fn partial_trick_evaluation_uses_the_same_suit_and_trump_rules() {
    let mode = Gamemode::Solo(Suit::Herz);
    let card = |suit, rank| Card { suit, rank };
    let cards = [
        card(Suit::Gras, Rank::Ass),
        card(Suit::Eichel, Rank::Zehn),
        card(Suit::Herz, Rank::Unter),
        card(Suit::Gras, Rank::Ober),
    ];
    assert_eq!(mode.winning_index(&[]), None);
    assert_eq!(mode.winning_index(&cards[..1]), Some(0));
    assert_eq!(mode.winning_index(&cards[..2]), Some(0));
    assert_eq!(mode.winning_index(&cards[..3]), Some(2));
    assert_eq!(mode.winning_index(&cards), Some(3));
    assert_eq!(mode.winning_card(cards.each_ref()), &cards[3]);
}

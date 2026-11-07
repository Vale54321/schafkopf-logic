use super::*;
use crate::deck::Suit;

fn config(mode: Gamemode) -> GameConfig {
    GameConfig {
        mode,
        first_player: PlayerId::ALL[0],
        declarer: PlayerId::ALL[2],
    }
}

#[test]
fn solo_threshold_is_61_and_the_declarer_is_independent_of_the_leader() {
    let config = config(Gamemode::Solo(Suit::Herz));
    let lost = result(config, None, [20, 20, 60, 20], [2; 4]);
    assert_eq!(
        lost.winners,
        vec![PlayerId::ALL[0], PlayerId::ALL[1], PlayerId::ALL[3]]
    );
    let won = result(config, None, [20, 20, 61, 19], [2; 4]);
    assert_eq!(won.winners, vec![config.declarer]);
}

#[test]
fn sauspiel_adds_the_partners_points_before_evaluating_the_threshold() {
    let config = config(Gamemode::Sauspiel(Suit::Gras));
    let partner = PlayerId::ALL[1];
    let won = result(config, Some(partner), [30, 31, 30, 29], [2; 4]);
    assert_eq!(won.winners, vec![partner, config.declarer]);
    assert!(matches!(
        won.reason,
        ResultReason::Points {
            team_points: 61,
            ..
        }
    ));
    let lost = result(config, Some(partner), [30, 30, 30, 30], [2; 4]);
    assert_eq!(lost.winners, vec![PlayerId::ALL[0], PlayerId::ALL[3]]);
}

#[test]
fn bettel_uses_trick_count_even_when_the_taken_trick_is_worth_zero_points() {
    let config = config(Gamemode::Bettel);
    assert!(result(config, None, [40, 40, 0, 40], [3, 3, 0, 2]).won(config.declarer));
    assert!(!result(config, None, [40, 40, 0, 40], [3, 2, 1, 2]).won(config.declarer));
}

#[test]
fn ramsch_highest_points_lose_including_ties() {
    let config = config(Gamemode::Ramsch);
    assert_eq!(
        result(config, None, [40, 40, 30, 10], [2; 4]).winners,
        vec![PlayerId::ALL[2], PlayerId::ALL[3]]
    );
    assert!(result(config, None, [30; 4], [2; 4]).winners.is_empty());
}

#[test]
fn ramsch_durchmarsch_requires_all_eight_tricks() {
    let config = config(Gamemode::Ramsch);
    let won = result(config, None, [0, 0, 120, 0], [0, 0, 8, 0]);
    assert_eq!(won.winners, vec![PlayerId::ALL[2]]);
    assert!(matches!(won.reason, ResultReason::Durchmarsch { .. }));
    assert!(!result(config, None, [0, 0, 120, 0], [0, 0, 7, 1]).won(PlayerId::ALL[2]));
}

#[test]
fn outcome_summaries_group_the_authoritative_winners_and_preserve_totals() {
    use crate::game::OutcomeSummary;
    let cases = [
        (Gamemode::Solo(Suit::Herz), None, [20, 20, 61, 19], [2; 4]),
        (Gamemode::Solo(Suit::Herz), None, [20, 20, 60, 20], [2; 4]),
        (
            Gamemode::Sauspiel(Suit::Gras),
            Some(PlayerId::ALL[1]),
            [30, 31, 30, 29],
            [2; 4],
        ),
        (Gamemode::Bettel, None, [40, 40, 0, 40], [3, 3, 0, 2]),
        (Gamemode::Bettel, None, [40, 40, 0, 40], [3, 2, 1, 2]),
        (Gamemode::Ramsch, None, [40, 40, 30, 10], [2; 4]),
        (Gamemode::Ramsch, None, [0, 0, 120, 0], [0, 0, 8, 0]),
    ];
    for (mode, partner, points, tricks) in cases {
        let result = result(config(mode), partner, points, tricks);
        let OutcomeSummary::Decided { winners, losers } = result.summary() else {
            panic!("{mode:?} should have a decided outcome");
        };
        assert_eq!(winners.players, result.winners);
        assert_eq!(winners.players.len() + losers.players.len(), 4);
        assert!(losers.players.iter().all(|p| !result.won(*p)));
        assert_eq!(winners.points + losers.points, 120);
        assert_eq!(winners.tricks + losers.tricks, 8);
        assert_eq!(
            winners.points,
            winners
                .players
                .iter()
                .map(|p| points[p.index()])
                .sum::<u16>()
        );
    }
    let OutcomeSummary::Draw(tied) =
        result(config(Gamemode::Ramsch), None, [30; 4], [2; 4]).summary()
    else {
        panic!("equal Ramsch points should be a draw");
    };
    assert_eq!(tied.players, PlayerId::ALL);
    assert_eq!((tied.points, tied.tricks), (120, 8));
}

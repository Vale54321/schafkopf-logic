use super::*;
use crate::{
    deck::{Card, Rank, Suit},
    game::{CompletedTrick, GameConfig, GamePhase, PlayedCard, PlayerId},
};
use Rank::*;
use Suit::*;

fn card(suit: Suit, rank: Rank) -> Card {
    Card { suit, rank }
}
fn play(seat: usize, suit: Suit, rank: Rank) -> PlayedCard {
    PlayedCard {
        player: PlayerId::ALL[seat],
        card: card(suit, rank),
    }
}
fn view(
    mode: Gamemode,
    actor: usize,
    declarer: usize,
    hand: Vec<Card>,
    trick: Vec<PlayedCard>,
) -> Observation {
    Observation {
        declarations: Vec::new(),
        player: PlayerId::ALL[actor],
        config: GameConfig {
            mode,
            first_player: PlayerId::ALL[0],
            declarer: PlayerId::ALL[declarer],
        },
        legal_actions: hand.iter().copied().map(Action::PlayCard).collect(),
        hand,
        hand_sizes: [3; 4],
        next_player: Some(PlayerId::ALL[actor]),
        trick,
        winner: None,
        phase: GamePhase::PlayingTrick,
        points: [0; 4],
        tricks_won: [0; 4],
        completed_tricks: 0,
        history: Vec::new(),
        partner: None,
        result: None,
    }
}
fn choice(view: &Observation) -> Card {
    let Action::PlayCard(card) = HeuristicNpc.choose_action(view).unwrap().action;
    card
}

#[test]
fn wins_a_valuable_trick_with_the_cheapest_sufficient_card() {
    let view = view(
        Gamemode::Solo(Herz),
        3,
        3,
        vec![
            card(Eichel, Ass),
            card(Eichel, Koenig),
            card(Eichel, Sieben),
        ],
        vec![
            play(0, Eichel, Zehn),
            play(1, Eichel, Acht),
            play(2, Eichel, Neun),
        ],
    );
    assert_eq!(choice(&view), card(Eichel, Ass));
    let mut cheaper = view.clone();
    cheaper.trick[0].card.rank = Neun;
    cheaper.trick[2].card = card(Schell, Sieben);
    assert_eq!(choice(&cheaper), card(Eichel, Koenig));
}

#[test]
fn does_not_overtrump_a_known_teammate_and_feeds_points_to_a_safe_winner() {
    let view = view(
        Gamemode::Solo(Herz),
        3,
        0,
        vec![card(Herz, Ass), card(Herz, Sieben)],
        vec![
            play(0, Herz, Acht),
            play(1, Eichel, Ober),
            play(2, Herz, Neun),
        ],
    );
    assert_eq!(choice(&view), card(Herz, Ass));
    let mut no_waste = view.clone();
    no_waste.hand = vec![card(Gras, Ober), card(Herz, Sieben)];
    no_waste.legal_actions = no_waste
        .hand
        .iter()
        .copied()
        .map(Action::PlayCard)
        .collect();
    no_waste.trick[1].card = card(Herz, Ober);
    assert_eq!(choice(&no_waste), card(Herz, Sieben));
}

#[test]
fn losing_to_an_opponent_preserves_points() {
    let view = view(
        Gamemode::Solo(Herz),
        3,
        3,
        vec![card(Gras, Ass), card(Gras, Sieben)],
        vec![
            play(0, Eichel, Ass),
            play(1, Eichel, Zehn),
            play(2, Eichel, Koenig),
        ],
    );
    assert_eq!(choice(&view), card(Gras, Sieben));
}

#[test]
fn bettel_declarer_sheds_a_dangerous_loser_and_defenders_force_the_declarer() {
    let declarer = view(
        Gamemode::Bettel,
        3,
        3,
        vec![card(Gras, Ober), card(Herz, Sieben), card(Eichel, Sieben)],
        vec![
            play(0, Eichel, Ober),
            play(1, Schell, Unter),
            play(2, Herz, Zehn),
        ],
    );
    assert_eq!(choice(&declarer), card(Gras, Ober));
    let defender = view(
        Gamemode::Bettel,
        3,
        1,
        vec![card(Eichel, Ober), card(Herz, Sieben)],
        vec![play(0, Herz, Acht), play(1, Herz, Ass), play(2, Herz, Zehn)],
    );
    assert_eq!(choice(&defender), card(Herz, Sieben));
    let mut must_avoid = declarer.clone();
    must_avoid.trick[0].card = card(Herz, Unter);
    must_avoid.hand = vec![card(Gras, Ober), card(Herz, Sieben)];
    must_avoid.legal_actions = must_avoid
        .hand
        .iter()
        .copied()
        .map(Action::PlayCard)
        .collect();
    assert_eq!(choice(&must_avoid), card(Herz, Sieben));
}

#[test]
fn ramsch_dumps_points_when_losing_and_avoids_an_expensive_trick() {
    let view = view(
        Gamemode::Ramsch,
        3,
        0,
        vec![card(Gras, Ass), card(Gras, Sieben)],
        vec![
            play(0, Eichel, Ass),
            play(1, Eichel, Zehn),
            play(2, Eichel, Koenig),
        ],
    );
    assert_eq!(choice(&view), card(Gras, Ass));
    let mut avoid = view.clone();
    avoid.hand = vec![card(Herz, Unter), card(Gras, Ass)];
    avoid.legal_actions = avoid.hand.iter().copied().map(Action::PlayCard).collect();
    assert_eq!(choice(&avoid), card(Gras, Ass));
}

#[test]
fn public_history_tracks_voids_and_excludes_seen_cards() {
    let mut view = view(
        Gamemode::Solo(Herz),
        0,
        0,
        vec![card(Eichel, Sieben)],
        vec![],
    );
    view.history.push(CompletedTrick {
        plays: [
            play(0, Gras, Zehn),
            play(1, Herz, Unter),
            play(2, Gras, Koenig),
            play(3, Schell, Acht),
        ],
        winner: PlayerId::ALL[1],
    });
    let known = knowledge::Knowledge::new(&view);
    let gras = 1 << knowledge::category(view.config.mode, card(Gras, Ass));
    assert_ne!(known.voids[1] & gras, 0);
    assert_ne!(known.voids[3] & gras, 0);
    assert_eq!(known.voids[2] & gras, 0);
    assert!(!known.unseen.contains(&card(Gras, Zehn)));
    assert!(!known.unseen.contains(&card(Eichel, Sieben)));
    assert_eq!(known.unseen.len(), 27);
}

#[test]
fn partnership_uses_only_information_available_to_the_observer() {
    let mut view = view(
        Gamemode::Sauspiel(Eichel),
        0,
        0,
        vec![card(Eichel, Sieben)],
        vec![],
    );
    assert_eq!(knowledge::ally(&view, PlayerId::ALL[1]), None);
    view.partner = Some(PlayerId::ALL[2]);
    assert_eq!(knowledge::ally(&view, PlayerId::ALL[2]), Some(true));
    assert_eq!(knowledge::ally(&view, PlayerId::ALL[1]), Some(false));
    view.player = PlayerId::ALL[2];
    assert_eq!(knowledge::ally(&view, PlayerId::ALL[0]), Some(true));
    view.player = PlayerId::ALL[1];
    view.partner = None;
    assert_eq!(knowledge::ally(&view, PlayerId::ALL[0]), Some(false));
    assert_eq!(knowledge::ally(&view, PlayerId::ALL[2]), None);
}

#[test]
fn completed_table_is_a_new_lead_and_finished_or_out_of_turn_policies_do_not_act() {
    let mut view = view(
        Gamemode::Solo(Herz),
        0,
        0,
        vec![card(Herz, Sieben)],
        vec![
            play(0, Eichel, Ass),
            play(1, Eichel, Zehn),
            play(2, Eichel, Koenig),
            play(3, Eichel, Sieben),
        ],
    );
    view.phase = GamePhase::TrickComplete;
    assert!(knowledge::active_trick(&view).is_empty());
    assert!(HeuristicNpc.choose_action(&view).is_some());
    view.next_player = None;
    assert!(HeuristicNpc.choose_action(&view).is_none());
    assert!(FirstLegal.choose_action(&view).is_none());
}

#[test]
fn a_known_void_makes_a_side_ace_more_likely_to_be_trumped() {
    let mut view = view(Gamemode::Solo(Herz), 0, 0, vec![card(Eichel, Ass)], vec![]);
    let plays = vec![play(0, Eichel, Ass)];
    let before = knowledge::Knowledge::new(&view).overtake(&view, &plays, PlayerId::ALL[1], false);
    view.history.push(CompletedTrick {
        plays: [
            play(0, Eichel, Sieben),
            play(1, Gras, Sieben),
            play(2, Eichel, Acht),
            play(3, Eichel, Neun),
        ],
        winner: PlayerId::ALL[3],
    });
    let after = knowledge::Knowledge::new(&view).overtake(&view, &plays, PlayerId::ALL[1], false);
    assert!(after > before);
}

#[test]
fn ramsch_prioritizes_achieving_or_preventing_durchmarsch_over_trick_points() {
    let mut view = view(
        Gamemode::Ramsch,
        3,
        0,
        vec![card(Herz, Unter), card(Gras, Sieben)],
        vec![
            play(0, Eichel, Ass),
            play(1, Gras, Ass),
            play(2, Schell, Ass),
        ],
    );
    assert_eq!(choice(&view), card(Gras, Sieben));
    view.completed_tricks = 6;
    view.tricks_won[3] = 6;
    assert_eq!(choice(&view), card(Herz, Unter));
    view.tricks_won = [6, 0, 0, 0];
    view.points[0] = 60;
    assert_eq!(choice(&view), card(Herz, Unter));
}

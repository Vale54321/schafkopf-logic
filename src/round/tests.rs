use super::*;
use crate::deck::{Rank, Suit};

fn complete(round: &mut Round) {
    while let Some(player) = round.observation(PlayerId::ALL[0]).next_player {
        let action = round.observation(player).legal_actions[0];
        round.play(player, action).unwrap();
    }
}

#[test]
fn four_passes_select_ramsch_without_changing_the_dealt_hands() {
    let mut round = Round::new(Deck::new(), PlayerId::ALL[0]).unwrap();
    let original: [_; 4] = std::array::from_fn(|i| round.observation(PlayerId::ALL[i]).hand);
    assert!(round.observation(PlayerId::ALL[0]).legal_actions.is_empty());
    assert!(
        round
            .play(PlayerId::ALL[0], Action::PlayCard(original[0][0]))
            .is_err()
    );
    for player in PlayerId::ALL {
        round.declare(player, None).unwrap();
    }
    assert_eq!(round.phase(), RoundPhase::Playing);
    assert_eq!(
        round.observation(PlayerId::ALL[0]).config.mode,
        Gamemode::Ramsch
    );
    assert_eq!(
        round.observation(PlayerId::ALL[0]).next_player,
        Some(PlayerId::ALL[0])
    );
    for i in 0..4 {
        assert_eq!(round.observation(PlayerId::ALL[i]).hand, original[i]);
    }
    complete(&mut round);
    assert_eq!(round.record().plays.len(), 32);
    assert_eq!(round.phase(), RoundPhase::Finished);
    assert_eq!(
        round
            .record()
            .result
            .as_ref()
            .unwrap()
            .points
            .iter()
            .sum::<u16>(),
        120
    );
}

#[test]
fn higher_declarations_replace_the_declarer_equal_ranks_and_invalid_calls_are_atomic() {
    let mut round = Round::new(Deck::new(), PlayerId::ALL[0]).unwrap();
    let before = round.record().clone();
    assert!(round.declare(PlayerId::ALL[1], None).is_err());
    assert!(
        round
            .declare(PlayerId::ALL[0], Some(Gamemode::Sauspiel(Suit::Herz)))
            .is_err()
    );
    assert!(
        round
            .declare(PlayerId::ALL[0], Some(Gamemode::Ramsch))
            .is_err()
    );
    assert_eq!(round.record(), &before);
    round
        .declare(PlayerId::ALL[0], Some(Gamemode::Sauspiel(Suit::Eichel)))
        .unwrap();
    round
        .declare(PlayerId::ALL[1], Some(Gamemode::Wenz(None)))
        .unwrap();
    let before = round.record().clone();
    assert_eq!(
        round.declare(PlayerId::ALL[2], Some(Gamemode::Geier(None))),
        Err(RoundError::MustOutbid)
    );
    assert_eq!(round.record(), &before);
    round
        .declare(PlayerId::ALL[2], Some(Gamemode::Solo(Suit::Gras)))
        .unwrap();
    round.declare(PlayerId::ALL[3], None).unwrap();
    let config = round.record().config.unwrap();
    assert_eq!(config.declarer, PlayerId::ALL[2]);
    assert_eq!(config.first_player, PlayerId::ALL[0]);
    assert_eq!(config.mode, Gamemode::Solo(Suit::Gras));
}

#[test]
fn npc_declarations_use_only_their_hand_and_finish_the_round() {
    for _ in 0..30 {
        let mut deck = Deck::new();
        deck.shuffle();
        let mut round = Round::new(deck, PlayerId::ALL[0]).unwrap();
        round.declare(PlayerId::ALL[0], None).unwrap();
        for player in &PlayerId::ALL[1..] {
            let choice = rating::npc_declaration(
                &round.observation(*player).hand,
                round.leading_bid().and_then(|bid| bid.mode),
            );
            assert!(choice.is_none_or(|mode| round.legal_declarations(*player).contains(&mode)));
            round.declare(*player, choice).unwrap();
        }
        complete(&mut round);
        assert_eq!(
            round
                .record()
                .result
                .as_ref()
                .unwrap()
                .points
                .iter()
                .sum::<u16>(),
            120
        );
    }
}

#[test]
fn json_contains_a_replayable_full_game_and_rejects_invalid_seats() {
    let mut round = Round::new(Deck::new(), PlayerId::ALL[0]).unwrap();
    round
        .declare(PlayerId::ALL[0], Some(Gamemode::Bettel))
        .unwrap();
    for player in &PlayerId::ALL[1..] {
        round.declare(*player, None).unwrap();
    }
    complete(&mut round);
    let json = serde_json::to_string_pretty(round.record()).unwrap();
    let record: RoundRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(&record, round.record());
    let mut replay = Round::new(
        Deck::from_deal_order(record.deal_order.clone()).unwrap(),
        record.first_player,
    )
    .unwrap();
    for declaration in &record.declarations {
        replay
            .declare(declaration.player, declaration.mode)
            .unwrap();
    }
    for play in &record.plays {
        assert_eq!(replay.play(play.player, play.action).unwrap(), play.events);
    }
    assert_eq!(replay.record(), &record);
    assert!(serde_json::from_str::<PlayerId>("4").is_err());
    assert!(serde_json::from_str::<PlayerId>("255").is_err());
    let mut duplicate = record.deal_order;
    duplicate[0] = duplicate[1];
    assert!(Deck::from_deal_order(duplicate).is_err());
    assert!(
        Deck::from_deal_order(vec![Card {
            suit: Suit::Herz,
            rank: Rank::Ass
        }])
        .is_err()
    );
}

#[test]
fn forcing_a_lower_contract_preserves_hands_and_opener_without_inventing_passes() {
    let mut round = Round::new(Deck::new(), PlayerId::ALL[1]).unwrap();
    let original: [_; 4] = std::array::from_fn(|i| round.observation(PlayerId::ALL[i]).hand);
    round
        .declare(PlayerId::ALL[1], Some(Gamemode::Solo(Suit::Herz)))
        .unwrap();
    round.declare(PlayerId::ALL[2], None).unwrap();
    round.declare(PlayerId::ALL[3], None).unwrap();
    assert!(round.legal_declarations(PlayerId::ALL[0]).is_empty());
    round
        .force_game(PlayerId::ALL[0], Gamemode::Sauspiel(Suit::Eichel))
        .unwrap();
    assert_eq!(round.phase(), RoundPhase::Playing);
    assert_eq!(round.record().schema_version, FORCED_RECORD_VERSION);
    assert_eq!(
        round.record().forced_contract,
        Some(ForcedContract {
            player: PlayerId::ALL[0],
            mode: Gamemode::Sauspiel(Suit::Eichel)
        })
    );
    assert_eq!(round.declarations().len(), 3);
    assert_eq!(round.record().declaration_decisions.len(), 3);
    assert_eq!(
        round.observation(PlayerId::ALL[0]).next_player,
        Some(PlayerId::ALL[1])
    );
    assert_eq!(round.record().config.unwrap().declarer, PlayerId::ALL[0]);
    for i in 0..4 {
        assert_eq!(round.observation(PlayerId::ALL[i]).hand, original[i]);
    }
    complete(&mut round);
    assert_eq!(round.record().plays.len(), 32);
    assert_eq!(
        round
            .record()
            .result
            .as_ref()
            .unwrap()
            .points
            .iter()
            .sum::<u16>(),
        120
    );
}

#[test]
fn forced_contracts_reject_wrong_turns_illegal_calls_and_changes_after_start() {
    let mut round = Round::new(Deck::new(), PlayerId::ALL[3]).unwrap();
    let before = round.record().clone();
    assert!(
        round
            .force_game(PlayerId::ALL[0], Gamemode::Bettel)
            .is_err()
    );
    assert!(
        round
            .force_game(PlayerId::ALL[3], Gamemode::Sauspiel(Suit::Eichel))
            .is_err()
    );
    assert!(
        round
            .force_game(PlayerId::ALL[3], Gamemode::Sauspiel(Suit::Herz))
            .is_err()
    );
    assert!(
        round
            .force_game(PlayerId::ALL[3], Gamemode::Ramsch)
            .is_err()
    );
    assert_eq!(round.record(), &before);
    round
        .force_game(PlayerId::ALL[3], Gamemode::Bettel)
        .unwrap();
    let before = round.record().clone();
    assert_eq!(
        round.force_game(PlayerId::ALL[3], Gamemode::Solo(Suit::Herz)),
        Err(RoundError::NotDeclaring)
    );
    assert_eq!(round.record(), &before);
}

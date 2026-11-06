use super::*;
use crate::{deck::Suit, gamemode::Gamemode};

fn config(first_player: PlayerId) -> GameConfig {
    GameConfig {
        mode: Gamemode::Solo(Suit::Herz),
        first_player,
    }
}

fn game() -> Game {
    Game::new(config(PlayerId::ALL[0]), Deck::new()).unwrap()
}

#[test]
fn player_ids_reject_invalid_seats() {
    for (index, player) in PlayerId::ALL.into_iter().enumerate() {
        assert_eq!(PlayerId::new(index), Some(player));
        assert_eq!(player.index(), index);
    }
    assert_eq!(PlayerId::new(4), None);
    assert_eq!(PlayerId::new(usize::MAX), None);
}

#[test]
fn deals_all_32_unique_cards_and_hides_other_hands() {
    let game = game();
    let mut all_cards = Vec::new();
    for player in PlayerId::ALL {
        let observation = game.observation(player);
        assert_eq!(observation.player, player);
        assert_eq!(observation.hand.len(), 8);
        assert_eq!(observation.hand_sizes, [8; 4]);
        assert_eq!(observation.next_player, Some(PlayerId::ALL[0]));
        assert!(observation.trick.is_empty());
        assert_eq!(observation.winner, None);
        for card in observation.hand {
            assert!(!all_cards.contains(&card));
            all_cards.push(card);
        }
    }
    assert_eq!(all_cards.len(), 32);
}

#[test]
fn observation_is_a_detached_snapshot() {
    let game = game();
    let before = game.clone();
    let mut observation = game.observation(PlayerId::ALL[0]);
    observation.hand.clear();
    observation.hand_sizes = [0; 4];
    assert_eq!(game, before);
}

#[test]
fn rejects_a_partially_consumed_deck() {
    let mut deck = Deck::new();
    deck.draw();
    assert_eq!(
        Game::new(config(PlayerId::ALL[0]), deck),
        Err(GameError::InvalidDeck {
            cards_remaining: 31
        })
    );
}

#[test]
fn rejects_out_of_turn_actions_without_mutation() {
    let mut game = game();
    let before = game.clone();
    let actual = PlayerId::ALL[1];
    let card = game.observation(actual).hand[0];
    assert_eq!(
        game.apply(actual, Action::PlayCard(card)),
        Err(GameError::WrongTurn {
            expected: PlayerId::ALL[0],
            actual,
        })
    );
    assert_eq!(game, before);
}

#[test]
fn rejects_cards_not_in_the_acting_hand_without_mutation() {
    let mut game = game();
    let before = game.clone();
    let player = PlayerId::ALL[0];
    let card = game.observation(PlayerId::ALL[1]).hand[0];
    assert_eq!(
        game.apply(player, Action::PlayCard(card)),
        Err(GameError::CardNotInHand { player, card })
    );
    assert_eq!(game, before);
}

#[test]
fn accepted_play_updates_only_its_hand_and_public_state() {
    let mut game = game();
    let player = PlayerId::ALL[0];
    let other_hands: Vec<_> = PlayerId::ALL[1..]
        .iter()
        .map(|&other| game.observation(other).hand)
        .collect();
    let card = game.observation(player).hand[0];
    let play = PlayedCard { player, card };
    assert_eq!(
        game.apply(player, Action::PlayCard(card)),
        Ok(vec![GameEvent::CardPlayed(play)])
    );
    let observation = game.observation(player);
    assert!(!observation.hand.contains(&card));
    assert_eq!(observation.hand_sizes, [7, 8, 8, 8]);
    assert_eq!(observation.trick, vec![play]);
    assert_eq!(game.next_player(), Some(PlayerId::ALL[1]));
    assert_eq!(game.phase(), GamePhase::PlayingTrick);
    for (&other, before) in PlayerId::ALL[1..].iter().zip(other_hands) {
        assert_eq!(game.observation(other).hand, before);
        assert_eq!(game.observation(other).trick, vec![play]);
    }
}

#[test]
fn completes_a_trick_from_each_leader_and_maps_winner_to_correct_seat() {
    for leader in PlayerId::ALL {
        let mut game = Game::new(config(leader), Deck::new()).unwrap();
        let mut plays = Vec::new();
        for offset in 0..4 {
            let player = PlayerId::ALL[(leader.index() + offset) % 4];
            assert_eq!(game.next_player(), Some(player));
            let Action::PlayCard(card) = game.legal_actions(player)[0];
            let play = PlayedCard { player, card };
            plays.push(play);
            let events = game.apply(player, Action::PlayCard(card)).unwrap();
            assert_eq!(events[0], GameEvent::CardPlayed(play));
            if offset < 3 {
                assert_eq!(events.len(), 1);
                assert_eq!(game.completed_trick(), None);
            } else {
                let cards = [plays[0].card, plays[1].card, plays[2].card, plays[3].card];
                let winning_card = config(leader).mode.winning_card(cards.each_ref());
                let winner = plays
                    .iter()
                    .find(|play| play.card == *winning_card)
                    .unwrap()
                    .player;
                let completed = CompletedTrick {
                    plays: plays.clone().try_into().unwrap(),
                    winner,
                };
                assert_eq!(
                    events,
                    vec![
                        GameEvent::CardPlayed(play),
                        GameEvent::TrickCompleted(completed)
                    ]
                );
                assert_eq!(game.completed_trick(), Some(completed));
                assert_eq!(game.phase(), GamePhase::TrickComplete);
                assert_eq!(game.next_player(), None);
                for observer in PlayerId::ALL {
                    let observation = game.observation(observer);
                    assert_eq!(observation.trick, plays);
                    assert_eq!(observation.hand_sizes, [7; 4]);
                    assert_eq!(observation.winner, Some(winner));
                }
            }
        }
        let before = game.clone();
        let card = game.observation(leader).hand[0];
        assert_eq!(
            game.apply(leader, Action::PlayCard(card)),
            Err(GameError::TrickAlreadyComplete)
        );
        assert_eq!(game, before);
    }
}

#[test]
fn identical_decks_and_actions_produce_identical_events_and_state() {
    let mut first = game();
    let mut second = game();
    for player in PlayerId::ALL {
        let action = first.legal_actions(player)[0];
        assert_eq!(first.apply(player, action), second.apply(player, action));
        assert_eq!(first, second);
    }
}

#[test]
fn legal_actions_are_only_exposed_for_the_acting_player() {
    let mut game = game();
    let player = PlayerId::ALL[0];
    let view = game.observation(player);
    assert_eq!(view.legal_cards().collect::<Vec<_>>(), view.hand);
    assert_eq!(view.legal_actions, game.legal_actions(player));
    for other in &PlayerId::ALL[1..] {
        assert!(game.legal_actions(*other).is_empty());
        assert!(game.observation(*other).legal_actions.is_empty());
    }
    game.apply(player, view.legal_actions[0]).unwrap();
    assert!(game.legal_actions(player).is_empty());
}

#[test]
fn rejects_trumping_a_suit_that_can_be_followed_without_mutation() {
    use crate::deck::{Rank::*, Suit::*};
    let mut game = game();
    game.apply(
        PlayerId::ALL[0],
        Action::PlayCard(Card {
            suit: Schell,
            rank: Sieben,
        }),
    )
    .unwrap();
    let before = game.clone();
    let player = PlayerId::ALL[1];
    assert_eq!(
        game.legal_actions(player),
        vec![
            Action::PlayCard(Card {
                suit: Schell,
                rank: Acht
            }),
            Action::PlayCard(Card {
                suit: Schell,
                rank: Koenig
            }),
        ]
    );
    assert_eq!(
        game.apply(
            player,
            Action::PlayCard(Card {
                suit: Herz,
                rank: Acht
            })
        ),
        Err(GameError::MustFollowSuit {
            player,
            suit: Schell
        })
    );
    assert_eq!(game, before);
}

#[test]
fn rejects_discarding_when_trump_was_led_and_trump_is_available() {
    use crate::deck::{Rank::*, Suit::*};
    let mut game = game();
    game.apply(
        PlayerId::ALL[0],
        Action::PlayCard(Card {
            suit: Eichel,
            rank: Ober,
        }),
    )
    .unwrap();
    let before = game.clone();
    let player = PlayerId::ALL[1];
    assert_eq!(
        game.legal_actions(player),
        vec![
            Action::PlayCard(Card {
                suit: Herz,
                rank: Acht
            }),
            Action::PlayCard(Card {
                suit: Herz,
                rank: Koenig
            }),
        ]
    );
    assert_eq!(
        game.apply(
            player,
            Action::PlayCard(Card {
                suit: Schell,
                rank: Acht
            })
        ),
        Err(GameError::MustFollowTrump { player })
    );
    assert_eq!(game, before);
}

#[test]
fn bettel_uses_hearts_solo_trumps_and_following_rules() {
    use crate::deck::{Rank::*, Suit::*};
    let mut config = config(PlayerId::ALL[0]);
    config.mode = Gamemode::Bettel;
    let mut game = Game::new(config, Deck::new()).unwrap();
    game.apply(
        PlayerId::ALL[0],
        Action::PlayCard(Card {
            suit: Eichel,
            rank: Ober,
        }),
    )
    .unwrap();
    let player = PlayerId::ALL[1];
    let before = game.clone();
    assert_eq!(
        game.apply(
            player,
            Action::PlayCard(Card {
                suit: Schell,
                rank: Acht
            })
        ),
        Err(GameError::MustFollowTrump { player })
    );
    assert_eq!(game, before);
    for (player, card) in [
        (
            PlayerId::ALL[1],
            Card {
                suit: Herz,
                rank: Acht,
            },
        ),
        (
            PlayerId::ALL[2],
            Card {
                suit: Herz,
                rank: Neun,
            },
        ),
        (
            PlayerId::ALL[3],
            Card {
                suit: Schell,
                rank: Unter,
            },
        ),
    ] {
        game.apply(player, Action::PlayCard(card)).unwrap();
    }
    assert_eq!(game.completed_trick().unwrap().winner, PlayerId::ALL[0]);
    assert_eq!(game.phase(), GamePhase::TrickComplete);
    assert!(game.legal_actions(PlayerId::ALL[0]).is_empty());
}

#[test]
fn every_supported_mode_completes_legal_tricks_from_every_leader_and_lead() {
    let suits = [Suit::Eichel, Suit::Gras, Suit::Herz, Suit::Schell];
    let mut modes = vec![
        Gamemode::Bettel,
        Gamemode::Wenz(None),
        Gamemode::Geier(None),
        Gamemode::Ramsch,
    ];
    for suit in suits {
        modes.extend([
            Gamemode::Solo(suit),
            Gamemode::Wenz(Some(suit)),
            Gamemode::Geier(Some(suit)),
        ]);
        if suit != Suit::Herz {
            modes.push(Gamemode::Sauspiel(suit));
        }
    }
    for mode in modes {
        for leader in PlayerId::ALL {
            for lead_index in 0..8 {
                let mut game = Game::new(
                    GameConfig {
                        mode,
                        first_player: leader,
                    },
                    Deck::new(),
                )
                .unwrap();
                let lead = game.legal_actions(leader)[lead_index];
                game.apply(leader, lead).unwrap();
                while let Some(player) = game.next_player() {
                    let view = game.observation(player);
                    assert!(!view.legal_actions.is_empty());
                    for candidate in &view.hand {
                        let mut candidate_game = game.clone();
                        let action = Action::PlayCard(*candidate);
                        let result = candidate_game.apply(player, action);
                        if view.legal_actions.contains(&action) {
                            assert!(result.is_ok(), "{mode:?}: {result:?}");
                        } else {
                            assert!(matches!(
                                result,
                                Err(GameError::MustFollowSuit { .. }
                                    | GameError::MustFollowTrump { .. })
                            ));
                            assert_eq!(candidate_game, game);
                        }
                    }
                    game.apply(player, view.legal_actions[0]).unwrap();
                }
                assert_eq!(game.phase(), GamePhase::TrickComplete);
                assert!(game.completed_trick().is_some());
                for player in PlayerId::ALL {
                    let view = game.observation(player);
                    assert!(view.legal_actions.is_empty());
                    assert_eq!(view.hand_sizes, [7; 4]);
                    assert_eq!(view.trick.len(), 4);
                }
            }
        }
    }
}

use super::*;
use crate::deck::{Rank::*, Suit::*};

fn hand(cards: &[(Suit, Rank)]) -> Vec<Card> {
    cards
        .iter()
        .map(|&(suit, rank)| Card { suit, rank })
        .collect()
}

#[test]
fn bettel_rewards_low_nontrumps_and_rejects_powerful_trump_hands() {
    let weak = hand(&[
        (Eichel, Sieben),
        (Eichel, Acht),
        (Eichel, Neun),
        (Gras, Sieben),
        (Gras, Acht),
        (Gras, Neun),
        (Schell, Sieben),
        (Schell, Acht),
    ]);
    let strong = hand(&[
        (Eichel, Ober),
        (Gras, Ober),
        (Herz, Ober),
        (Schell, Ober),
        (Eichel, Unter),
        (Gras, Unter),
        (Herz, Ass),
        (Herz, Zehn),
    ]);
    assert!(rate_hand(&weak, Gamemode::Bettel).score >= 90);
    assert_eq!(rate_hand(&strong, Gamemode::Bettel).score, 0);
    assert_eq!(npc_declaration(&weak, None), Some(Gamemode::Bettel));
    assert!(matches!(
        npc_declaration(&strong, None),
        Some(Gamemode::Solo(_))
    ));
    let mut dangerous = weak.clone();
    dangerous[0] = Card {
        suit: Eichel,
        rank: Ober,
    };
    assert!(rate_hand(&dangerous, Gamemode::Bettel).score < BETTEL_MIN_SCORE);
}

#[test]
fn suit_choice_depends_on_trumps_and_sauspiel_requires_a_legal_call() {
    let cards = hand(&[
        (Eichel, Ober),
        (Gras, Ober),
        (Eichel, Unter),
        (Gras, Unter),
        (Gras, Ass),
        (Gras, Zehn),
        (Gras, Koenig),
        (Gras, Sieben),
    ]);
    assert!(
        rate_hand(&cards, Gamemode::Solo(Gras)).score
            > rate_hand(&cards, Gamemode::Solo(Schell)).score
    );
    assert!(!rate_hand(&cards, Gamemode::Sauspiel(Gras)).eligible);
    assert!(!rate_hand(&cards, Gamemode::Sauspiel(Eichel)).eligible); // only trump in Eichel
    assert!(!rate_hand(&cards, Gamemode::Sauspiel(Herz)).eligible);
    assert!(ratings(&cards).iter().all(|r| r.score <= 100));
    assert_eq!(npc_declaration(&cards, Some(Gamemode::Solo(Herz))), None);
}

#[test]
fn a_mediocre_hand_passes_and_strength_does_not_leak_other_hands() {
    let mediocre = hand(&[
        (Eichel, Koenig),
        (Eichel, Neun),
        (Gras, Koenig),
        (Gras, Neun),
        (Schell, Koenig),
        (Schell, Neun),
        (Herz, Sieben),
        (Herz, Acht),
    ]);
    assert_eq!(npc_declaration(&mediocre, None), None);
    assert_eq!(ratings(&mediocre), ratings(&mediocre));
}

//! Hand-only heuristics. Scores are suitability estimates, not win probabilities.

use crate::{
    deck::{Card, Rank, Suit},
    gamemode::Gamemode,
};
use serde::{Deserialize, Serialize};

pub const NPC_MIN_SCORE: u8 = 65;
pub const BETTEL_MIN_SCORE: u8 = 80;

pub const DECLARABLE_MODES: [Gamemode; 18] = [
    Gamemode::Solo(Suit::Eichel),
    Gamemode::Solo(Suit::Gras),
    Gamemode::Solo(Suit::Herz),
    Gamemode::Solo(Suit::Schell),
    Gamemode::Sauspiel(Suit::Eichel),
    Gamemode::Sauspiel(Suit::Gras),
    Gamemode::Sauspiel(Suit::Schell),
    Gamemode::Wenz(None),
    Gamemode::Wenz(Some(Suit::Eichel)),
    Gamemode::Wenz(Some(Suit::Gras)),
    Gamemode::Wenz(Some(Suit::Herz)),
    Gamemode::Wenz(Some(Suit::Schell)),
    Gamemode::Geier(None),
    Gamemode::Geier(Some(Suit::Eichel)),
    Gamemode::Geier(Some(Suit::Gras)),
    Gamemode::Geier(Some(Suit::Herz)),
    Gamemode::Geier(Some(Suit::Schell)),
    Gamemode::Bettel,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandRating {
    pub mode: Gamemode,
    pub score: u8,
    pub eligible: bool,
    pub explanation: String,
}

/// House bidding order: Solo > pure Wenz/Geier > suit Wenz/Geier > Bettel > Sauspiel.
/// Suits have equal priority; an earlier declaration keeps an equal-ranked game.
pub fn priority(mode: Gamemode) -> u8 {
    match mode {
        Gamemode::Ramsch => 0,
        Gamemode::Sauspiel(_) => 1,
        Gamemode::Bettel => 2,
        Gamemode::Wenz(Some(_)) | Gamemode::Geier(Some(_)) => 3,
        Gamemode::Wenz(None) | Gamemode::Geier(None) => 4,
        Gamemode::Solo(_) => 5,
    }
}

pub fn eligible(hand: &[Card], mode: Gamemode) -> bool {
    match mode {
        Gamemode::Sauspiel(suit) => {
            suit != Suit::Herz
                && !hand.contains(&Card {
                    suit,
                    rank: Rank::Ass,
                })
                && hand
                    .iter()
                    .any(|card| card.suit == suit && !mode.is_trump(card))
        }
        Gamemode::Ramsch => false,
        _ => true,
    }
}

pub fn rate_hand(hand: &[Card], mode: Gamemode) -> HandRating {
    let can_play = eligible(hand, mode);
    let trumps = hand.iter().filter(|card| mode.is_trump(card)).count();
    let aces = hand
        .iter()
        .filter(|card| !mode.is_trump(card) && card.rank == Rank::Ass)
        .count();
    let (score, explanation) = if mode == Gamemode::Bettel {
        // This house Bettel uses Herz Solo trumps. Even a zero-point trump
        // can force a trick, so card points alone are not a useful rating.
        let risk: i32 = hand
            .iter()
            .map(|card| match card.rank {
                Rank::Ober => 24 + suit_strength(card.suit),
                Rank::Unter => 18 + suit_strength(card.suit),
                rank if card.suit == Suit::Herz => match rank {
                    Rank::Ass => 22,
                    Rank::Zehn => 18,
                    Rank::Koenig => 14,
                    Rank::Neun => 10,
                    Rank::Acht => 8,
                    _ => 6,
                },
                Rank::Ass => 20,
                Rank::Zehn => 13,
                Rank::Koenig => 8,
                Rank::Neun => 3,
                Rank::Acht => 1,
                Rank::Sieben => 0,
            })
            .sum();
        (
            (100 - risk).clamp(0, 100),
            format!(
                "Avoid tricks: {trumps} trumps, {aces} side aces; high cards lower this score."
            ),
        )
    } else {
        let mut strength: i32 = hand
            .iter()
            .filter(|card| mode.is_trump(card))
            .map(|card| match mode {
                Gamemode::Wenz(_) if card.rank == Rank::Unter => 11 + 3 * suit_strength(card.suit),
                Gamemode::Geier(_) if card.rank == Rank::Ober => 11 + 3 * suit_strength(card.suit),
                _ => match card.rank {
                    Rank::Ober => 8 + 2 * suit_strength(card.suit),
                    Rank::Unter => 6 + suit_strength(card.suit),
                    Rank::Ass => 10,
                    Rank::Zehn => 8,
                    Rank::Koenig => 5,
                    _ => 3,
                },
            })
            .sum();
        strength += aces as i32 * 10;
        for suit in [Suit::Eichel, Suit::Gras, Suit::Herz, Suit::Schell] {
            let has = |rank| hand.contains(&Card { suit, rank });
            if has(Rank::Ass)
                && has(Rank::Zehn)
                && !mode.is_trump(&Card {
                    suit,
                    rank: Rank::Zehn,
                })
            {
                strength += 5;
            }
        }
        strength += trumps.saturating_sub(4) as i32 * 3;
        if matches!(mode, Gamemode::Sauspiel(_)) {
            strength += 15;
        }
        (
            strength.clamp(0, 100),
            format!(
                "{trumps} trumps, {aces} side aces; strong trumps and protected tens raise this score."
            ),
        )
    };
    HandRating {
        mode,
        score: score as u8,
        eligible: can_play,
        explanation,
    }
}

fn suit_strength(suit: Suit) -> i32 {
    match suit {
        Suit::Eichel => 4,
        Suit::Gras => 3,
        Suit::Herz => 2,
        Suit::Schell => 1,
    }
}

pub fn ratings(hand: &[Card]) -> Vec<HandRating> {
    DECLARABLE_MODES
        .into_iter()
        .map(|mode| rate_hand(hand, mode))
        .collect()
}

/// Looks only at this hand and the public leading bid. Returns None to pass.
pub fn npc_declaration(hand: &[Card], leading: Option<Gamemode>) -> Option<Gamemode> {
    ratings(hand)
        .into_iter()
        .filter(|rating| {
            rating.eligible
                && rating.score
                    >= if rating.mode == Gamemode::Bettel {
                        BETTEL_MIN_SCORE
                    } else {
                        NPC_MIN_SCORE
                    }
                && leading.is_none_or(|mode| priority(rating.mode) > priority(mode))
        })
        .max_by_key(|rating| (rating.score, priority(rating.mode)))
        .map(|rating| rating.mode)
}

#[cfg(test)]
mod tests;

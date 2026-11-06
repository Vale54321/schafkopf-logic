//! Ordinary following-suit rules. Sauspiel called-ace restrictions need match
//! and partnership state and are deliberately outside this one-trick phase.

use crate::{deck::Card, gamemode::Gamemode};

fn follows(mode: Gamemode, card: Card, lead: Card) -> bool {
    if mode.is_trump(&lead) {
        mode.is_trump(&card)
    } else {
        !mode.is_trump(&card) && card.suit == lead.suit
    }
}

pub(super) fn legal_cards(mode: Gamemode, hand: &[Card], lead: Option<Card>) -> Vec<Card> {
    let Some(lead) = lead else {
        return hand.to_vec();
    };
    let following: Vec<_> = hand
        .iter()
        .copied()
        .filter(|card| follows(mode, *card, lead))
        .collect();
    if following.is_empty() {
        hand.to_vec()
    } else {
        following
    }
}

#[cfg(test)]
mod tests;

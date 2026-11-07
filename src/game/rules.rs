//! Ordinary suit/trump following rules. Called-ace restrictions are layered
//! on top by the separate Sauspiel module.

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

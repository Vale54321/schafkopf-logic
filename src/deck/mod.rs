mod rank;
pub use rank::Rank;

mod suit;
pub use suit::Suit;

mod card;
pub use card::Card;

use strum::IntoEnumIterator;

use rand::rng;
use rand::seq::SliceRandom;

#[derive(Clone)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Default for Deck {
    fn default() -> Self {
        Self::new()
    }
}

impl Deck {
    /// A complete deck in chronological deal order (seat 0, seat 1, ...).
    /// Rejects missing or repeated cards so saved games can be reconstructed.
    pub fn from_deal_order(mut cards: Vec<Card>) -> Result<Self, &'static str> {
        if cards.len() != 32
            || cards
                .iter()
                .enumerate()
                .any(|(i, card)| cards[..i].contains(card))
        {
            return Err("a deck must contain all 32 distinct cards");
        }
        cards.reverse();
        Ok(Self { cards })
    }

    pub fn deal_order(&self) -> Vec<Card> {
        self.cards.iter().rev().copied().collect()
    }

    pub fn new() -> Self {
        let cards = Suit::iter()
            .flat_map(|suit| Rank::iter().map(move |rank| Card { suit, rank }))
            .collect();
        Self { cards }
    }

    pub fn draw(&mut self) -> Option<Card> {
        self.cards.pop()
    }

    pub fn shuffle(&mut self) {
        self.cards.shuffle(&mut rng());
    }

    pub fn deal_4x8(&mut self) -> Option<[Vec<Card>; 4]> {
        if self.cards.len() < 32 {
            return None;
        }
        let mut hands = [
            Vec::with_capacity(8),
            Vec::with_capacity(8),
            Vec::with_capacity(8),
            Vec::with_capacity(8),
        ];
        for _ in 0..8 {
            for h in 0..4 {
                hands[h].push(self.draw()?);
            }
        }
        Some(hands)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Card> {
        self.cards.iter()
    }
}

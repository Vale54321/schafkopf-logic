use serde::{Deserialize, Serialize};
use std::fmt;

use crate::deck::{Rank, Suit};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.suit, self.rank)
    }
}

impl Card {
    /// Stable recording ID: suit * 8 + rank. Never depends on enum discriminants.
    pub fn id(self) -> u8 {
        let suit = match self.suit {
            Suit::Eichel => 0,
            Suit::Gras => 1,
            Suit::Herz => 2,
            Suit::Schell => 3,
        };
        let rank = match self.rank {
            Rank::Ass => 0,
            Rank::Zehn => 1,
            Rank::Koenig => 2,
            Rank::Ober => 3,
            Rank::Unter => 4,
            Rank::Neun => 5,
            Rank::Acht => 6,
            Rank::Sieben => 7,
        };
        suit * 8 + rank
    }

    pub fn from_id(id: u8) -> Option<Self> {
        let suits = [Suit::Eichel, Suit::Gras, Suit::Herz, Suit::Schell];
        let ranks = [
            Rank::Ass,
            Rank::Zehn,
            Rank::Koenig,
            Rank::Ober,
            Rank::Unter,
            Rank::Neun,
            Rank::Acht,
            Rank::Sieben,
        ];
        Some(Self {
            suit: *suits.get(usize::from(id / 8))?,
            rank: ranks[usize::from(id % 8)],
        })
    }
}

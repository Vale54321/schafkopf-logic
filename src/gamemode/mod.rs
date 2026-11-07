use crate::deck::{Card, Rank, Suit};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Gamemode {
    Sauspiel(Suit),
    Solo(Suit),
    Wenz(Option<Suit>),
    Geier(Option<Suit>),
    /// House variant: Herz Solo trumps and card ordering, but the soloist
    /// aims to take no tricks.
    Bettel,
    Ramsch,
}

impl Gamemode {
    /// Trump first, strongest to weakest, then Eichel/Gras/Herz/Schell with
    /// A > 10 > K > O > U > 9 > 8 > 7 for the remaining non-trump cards.
    /// Frontends should sort a copy of the observed hand for presentation.
    pub fn sort_hand(&self, hand: &mut [Card]) {
        hand.sort_by_key(|card| {
            let strength = self.trump_strength(card);
            (
                !self.is_trump(card),
                std::cmp::Reverse(strength),
                card.suit,
                card.rank,
            )
        });
    }

    /// Shared classification for trick evaluation and engine move validation.
    pub fn is_trump(&self, card: &Card) -> bool {
        match self {
            Self::Solo(suit) => is_trump(card, &[Rank::Ober, Rank::Unter], Some(*suit)),
            Self::Wenz(suit) => is_trump(card, &[Rank::Unter], *suit),
            Self::Geier(suit) => is_trump(card, &[Rank::Ober], *suit),
            Self::Sauspiel(_) | Self::Ramsch | Self::Bettel => {
                is_trump(card, &[Rank::Ober, Rank::Unter], Some(Suit::Herz))
            }
        }
    }

    /// Index of the strongest card in a nonempty partial or complete trick.
    pub fn winning_index(&self, cards: &[Card]) -> Option<usize> {
        let lead = cards.first()?;
        let trump_played = cards.iter().any(|card| self.is_trump(card));
        cards
            .iter()
            .enumerate()
            .filter(|(_, card)| {
                if trump_played {
                    self.is_trump(card)
                } else {
                    card.suit == lead.suit
                }
            })
            .max_by_key(|(_, card)| {
                if trump_played {
                    self.trump_strength(card)
                } else {
                    u16::from(non_trump_strength(card.rank))
                }
            })
            .map(|(index, _)| index)
    }

    pub fn winning_card<'a>(&self, cards: [&'a Card; 4]) -> &'a Card {
        cards[self
            .winning_index(&cards.map(|card| *card))
            .expect("a trick contains four cards")]
    }

    pub(crate) fn trump_strength(&self, card: &Card) -> u16 {
        match self {
            Self::Solo(suit) => trump_strength(card, *suit),
            Self::Wenz(suit) => trump_strength_wenz(card, Rank::Unter, *suit),
            Self::Geier(suit) => trump_strength_wenz(card, Rank::Ober, *suit),
            Self::Sauspiel(_) | Self::Ramsch | Self::Bettel => trump_strength(card, Suit::Herz),
        }
    }
}

fn is_trump(card: &Card, trump_ranks: &[Rank], trump_suit: Option<Suit>) -> bool {
    trump_ranks.contains(&card.rank) || (trump_suit == Some(card.suit))
}

// Trump strength according to Schafkopf:
// Obers: Eichel > Gras > Herz > Schell
// Unters: Eichel > Gras > Herz > Schell
// Then trump suit cards: A > 10 > K > 9 > 8 > 7
fn trump_strength(card: &Card, trump_suit: Suit) -> u16 {
    match card.rank {
        Rank::Ober => 300 + ober_unter_suit_strength(card.suit),
        Rank::Unter => 200 + ober_unter_suit_strength(card.suit),
        _ if card.suit == trump_suit => 100 + non_trump_strength(card.rank) as u16,
        _ => 0,
    }
}

fn trump_strength_wenz(card: &Card, rank: Rank, trump_suit: Option<Suit>) -> u16 {
    if card.rank == rank {
        200 + ober_unter_suit_strength(card.suit)
    } else if trump_suit == Some(card.suit) {
        100 + non_trump_strength(card.rank) as u16
    } else {
        0
    }
}

fn ober_unter_suit_strength(suit: Suit) -> u16 {
    match suit {
        Suit::Eichel => 4,
        Suit::Gras => 3,
        Suit::Herz => 2,
        Suit::Schell => 1,
    }
}

fn non_trump_strength(rank: Rank) -> u8 {
    match rank {
        Rank::Ass => 8,
        Rank::Zehn => 7,
        Rank::Koenig => 6,
        Rank::Ober => 5,
        Rank::Unter => 4,
        Rank::Neun => 3,
        Rank::Acht => 2,
        Rank::Sieben => 1,
    }
}

#[cfg(test)]
mod tests;

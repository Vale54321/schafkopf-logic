use crate::deck::{Card, Rank, Suit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gamemode {
    Sauspiel(Suit),
    Solo(Suit),
    Wenz(Option<Suit>),
    Geier(Option<Suit>),
    /// House variant: Herz Solo trumps and card ordering, but the soloist
    /// aims to take no tricks (full-match results are handled separately).
    Bettel,
    Ramsch,
}

impl Gamemode {
    /// Shared classification for trick evaluation and engine move validation.
    pub(crate) fn is_trump(&self, card: &Card) -> bool {
        match self {
            Self::Solo(suit) => is_trump(card, &[Rank::Ober, Rank::Unter], Some(*suit)),
            Self::Wenz(suit) => is_trump(card, &[Rank::Unter], *suit),
            Self::Geier(suit) => is_trump(card, &[Rank::Ober], *suit),
            Self::Sauspiel(_) | Self::Ramsch | Self::Bettel => {
                is_trump(card, &[Rank::Ober, Rank::Unter], Some(Suit::Herz))
            }
        }
    }

    pub fn winning_card<'a>(&self, cards: [&'a Card; 4]) -> &'a Card {
        match self {
            Gamemode::Sauspiel(_) | Gamemode::Ramsch | Gamemode::Bettel => {
                winner_for_trump(self, Suit::Herz, cards)
            }
            Gamemode::Solo(solo_suit) => winner_for_trump(self, *solo_suit, cards),
            Gamemode::Wenz(wenz_suit) => winner_for_wenz(self, Rank::Unter, *wenz_suit, cards),
            Gamemode::Geier(geier_suit) => winner_for_wenz(self, Rank::Ober, *geier_suit, cards),
        }
    }
}

fn winner_for_wenz<'a>(
    mode: &Gamemode,
    rank: Rank,
    trump_suit: Option<Suit>,
    cards: [&'a Card; 4],
) -> &'a Card {
    if cards.iter().any(|&c| mode.is_trump(c)) {
        let winner_idx = cards
            .iter()
            .enumerate()
            .filter(|&(_, &c)| mode.is_trump(c))
            .max_by_key(|&(_, &c)| trump_strength_wenz(c, rank, trump_suit))
            .map(|(i, _)| i)
            .unwrap_or(0);
        cards[winner_idx]
    } else {
        let first_suit = cards[0].suit;
        let winner_idx = cards
            .iter()
            .enumerate()
            .filter(|&(_, &c)| c.suit == first_suit)
            .max_by_key(|&(_, &c)| non_trump_strength(c.rank))
            .map(|(i, _)| i)
            .unwrap_or(0);
        cards[winner_idx]
    }
}

fn winner_for_trump<'a>(mode: &Gamemode, trump_suit: Suit, cards: [&'a Card; 4]) -> &'a Card {
    if cards.iter().any(|&c| mode.is_trump(c)) {
        // Highest trump wins
        let winner_idx = cards
            .iter()
            .enumerate()
            .filter(|&(_, &c)| mode.is_trump(c))
            .max_by_key(|&(_, &c)| trump_strength(c, trump_suit))
            .map(|(i, _)| i)
            .unwrap_or(0);
        cards[winner_idx]
    } else {
        // No trump: highest of the led suit wins
        let first_suit = cards[0].suit;
        let winner_idx = cards
            .iter()
            .enumerate()
            .filter(|&(_, &c)| c.suit == first_suit)
            .max_by_key(|&(_, &c)| non_trump_strength(c.rank))
            .map(|(i, _)| i)
            .unwrap_or(0);
        cards[winner_idx]
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

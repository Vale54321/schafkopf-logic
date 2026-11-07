use super::{GameError, PlayerId, SauspielError};
use crate::{
    deck::{Card, Rank, Suit},
    gamemode::Gamemode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Sauspiel {
    suit: Suit,
    pub partner: PlayerId,
    escaped: bool,
    ace_played: bool,
}

impl Sauspiel {
    pub fn new(suit: Suit, declarer: PlayerId, hands: &[Vec<Card>; 4]) -> Result<Self, GameError> {
        let invalid = |reason| GameError::InvalidSauspiel(reason);
        if suit == Suit::Herz {
            return Err(invalid(SauspielError::HeartsCannotBeCalled));
        }
        let ace = Card {
            suit,
            rank: Rank::Ass,
        };
        let hand = &hands[declarer.index()];
        if hand.contains(&ace) {
            return Err(invalid(SauspielError::DeclarerHoldsCalledAce));
        }
        if !hand.iter().any(|card| Self::is_called_suit(suit, *card)) {
            return Err(invalid(SauspielError::DeclarerLacksCalledSuit));
        }
        let partner = PlayerId::ALL
            .into_iter()
            .find(|player| hands[player.index()].contains(&ace))
            .expect("a full deck contains the called ace");
        Ok(Self {
            suit,
            partner,
            escaped: false,
            ace_played: false,
        })
    }

    fn is_called_suit(suit: Suit, card: Card) -> bool {
        card.suit == suit && !Gamemode::Sauspiel(suit).is_trump(&card)
    }

    fn ace(&self) -> Card {
        Card {
            suit: self.suit,
            rank: Rank::Ass,
        }
    }

    pub fn restrict(
        &self,
        player: PlayerId,
        hand: &[Card],
        lead: Option<Card>,
        last_trick: bool,
        cards: Vec<Card>,
    ) -> Vec<Card> {
        if player != self.partner || self.ace_played || self.escaped {
            return cards;
        }
        let ace = self.ace();
        match lead {
            Some(lead) if Self::is_called_suit(self.suit, lead) => vec![ace],
            Some(_) if !last_trick => cards.into_iter().filter(|card| *card != ace).collect(),
            None if hand
                .iter()
                .filter(|card| Self::is_called_suit(self.suit, **card))
                .count()
                < 4 =>
            {
                cards
                    .into_iter()
                    .filter(|card| *card == ace || !Self::is_called_suit(self.suit, *card))
                    .collect()
            }
            _ => cards,
        }
    }

    pub fn played(&mut self, player: PlayerId, card: Card, leading: bool) {
        if player == self.partner
            && leading
            && Self::is_called_suit(self.suit, card)
            && card != self.ace()
        {
            self.escaped = true;
        }
        if card == self.ace() {
            self.ace_played = true;
        }
    }

    pub fn visible_partner(&self, observer: PlayerId) -> Option<PlayerId> {
        if self.ace_played || observer == self.partner {
            Some(self.partner)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;

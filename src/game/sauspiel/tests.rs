use super::*;
use crate::deck::{Rank::*, Suit::*};

fn card(suit: Suit, rank: Rank) -> Card {
    Card { suit, rank }
}
fn state(partner_hand: Vec<Card>) -> Sauspiel {
    Sauspiel::new(
        Eichel,
        PlayerId::ALL[0],
        &[vec![card(Eichel, Sieben)], partner_hand, vec![], vec![]],
    )
    .unwrap()
}

#[test]
fn validates_the_called_suit_and_declarers_hand() {
    let hands = [
        vec![card(Eichel, Ass)],
        vec![card(Gras, Ass)],
        vec![],
        vec![],
    ];
    assert_eq!(
        Sauspiel::new(Herz, PlayerId::ALL[0], &hands),
        Err(GameError::InvalidSauspiel(
            SauspielError::HeartsCannotBeCalled
        ))
    );
    assert_eq!(
        Sauspiel::new(Eichel, PlayerId::ALL[0], &hands),
        Err(GameError::InvalidSauspiel(
            SauspielError::DeclarerHoldsCalledAce
        ))
    );
    assert_eq!(
        Sauspiel::new(Gras, PlayerId::ALL[0], &hands),
        Err(GameError::InvalidSauspiel(
            SauspielError::DeclarerLacksCalledSuit
        ))
    );
}

#[test]
fn called_ace_must_follow_its_suit_and_cannot_be_discarded_early() {
    let hand = vec![card(Eichel, Ass), card(Eichel, Koenig), card(Gras, Sieben)];
    let state = state(hand.clone());
    let partner = state.partner;
    assert_eq!(
        state.restrict(
            partner,
            &hand,
            Some(card(Eichel, Sieben)),
            false,
            hand[..2].to_vec()
        ),
        vec![hand[0]]
    );
    assert_eq!(
        state.restrict(partner, &hand, Some(card(Herz, Ober)), false, hand.clone()),
        hand[1..]
    );
    assert_eq!(
        state.restrict(
            partner,
            &[hand[0]],
            Some(card(Herz, Ober)),
            true,
            vec![hand[0]]
        ),
        vec![hand[0]]
    );
}

#[test]
fn leading_below_the_called_ace_requires_four_called_suit_cards() {
    let short = vec![card(Eichel, Ass), card(Eichel, Koenig), card(Gras, Sieben)];
    let s = state(short.clone());
    assert_eq!(
        s.restrict(s.partner, &short, None, false, short.clone()),
        vec![short[0], short[2]]
    );
    let long = vec![
        card(Eichel, Ass),
        card(Eichel, Koenig),
        card(Eichel, Zehn),
        card(Eichel, Sieben),
    ];
    let mut s = state(long.clone());
    assert_eq!(
        s.restrict(s.partner, &long, None, false, long.clone()),
        long
    );
    s.played(s.partner, card(Eichel, Koenig), true);
    let remaining = vec![card(Eichel, Ass), card(Eichel, Zehn), card(Eichel, Sieben)];
    assert_eq!(
        s.restrict(
            s.partner,
            &remaining,
            Some(card(Eichel, Acht)),
            false,
            remaining.clone()
        ),
        remaining
    );
    assert_eq!(
        s.restrict(
            s.partner,
            &remaining,
            Some(card(Herz, Ober)),
            false,
            remaining.clone()
        ),
        remaining
    );
}

#[test]
fn partnership_is_hidden_until_the_ace_is_played_except_to_its_holder() {
    let mut s = state(vec![card(Eichel, Ass)]);
    assert_eq!(s.visible_partner(PlayerId::ALL[0]), None);
    assert_eq!(s.visible_partner(s.partner), Some(s.partner));
    s.played(s.partner, card(Eichel, Ass), false);
    for player in PlayerId::ALL {
        assert_eq!(s.visible_partner(player), Some(s.partner));
    }
}

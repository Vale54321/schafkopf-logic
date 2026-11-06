use super::*;
use crate::deck::{Rank::*, Suit::*};

#[test]
fn bettel_uses_hearts_solo_categories_instead_of_printed_suits() {
    let hand = [
        Card {
            suit: Eichel,
            rank: Ober,
        },
        Card {
            suit: Eichel,
            rank: Unter,
        },
        Card {
            suit: Eichel,
            rank: Sieben,
        },
        Card {
            suit: Herz,
            rank: Ass,
        },
    ];
    assert_eq!(
        legal_cards(
            Gamemode::Bettel,
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Zehn
            })
        ),
        vec![hand[2]]
    );
    assert_eq!(
        legal_cards(
            Gamemode::Bettel,
            &hand,
            Some(Card {
                suit: Herz,
                rank: Ober
            })
        ),
        vec![hand[0], hand[1], hand[3]]
    );
}

#[test]
fn bettel_allows_any_card_when_void_and_does_not_force_overtaking() {
    let hand = [
        Card {
            suit: Gras,
            rank: Ass,
        },
        Card {
            suit: Gras,
            rank: Sieben,
        },
        Card {
            suit: Herz,
            rank: Unter,
        },
    ];
    assert_eq!(
        legal_cards(
            Gamemode::Bettel,
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Ass
            })
        ),
        hand
    );
    assert_eq!(
        legal_cards(
            Gamemode::Bettel,
            &hand,
            Some(Card {
                suit: Gras,
                rank: Koenig
            })
        ),
        vec![hand[0], hand[1]]
    );
    // Trump must be followed even when only a lower trump is available.
    assert_eq!(
        legal_cards(
            Gamemode::Bettel,
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Ober
            })
        ),
        vec![hand[2]]
    );
}

#[test]
fn trump_faces_do_not_follow_their_printed_suit_in_solo() {
    let hand = [
        Card {
            suit: Eichel,
            rank: Ober,
        },
        Card {
            suit: Eichel,
            rank: Unter,
        },
        Card {
            suit: Eichel,
            rank: Sieben,
        },
        Card {
            suit: Herz,
            rank: Ass,
        },
    ];
    let mode = Gamemode::Solo(Herz);
    assert_eq!(
        legal_cards(
            mode,
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Ass
            })
        ),
        vec![hand[2]]
    );
    assert_eq!(
        legal_cards(
            mode,
            &hand,
            Some(Card {
                suit: Gras,
                rank: Ober
            })
        ),
        vec![hand[0], hand[1], hand[3]]
    );
}

#[test]
fn void_players_may_discard_or_trump_without_being_forced_to_win() {
    let hand = [
        Card {
            suit: Gras,
            rank: Sieben,
        },
        Card {
            suit: Herz,
            rank: Sieben,
        },
    ];
    assert_eq!(
        legal_cards(
            Gamemode::Solo(Herz),
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Ass
            })
        ),
        hand
    );
    // Holding a lower trump is sufficient; there is no requirement to beat the lead.
    assert_eq!(
        legal_cards(
            Gamemode::Solo(Herz),
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Ober
            })
        ),
        vec![hand[1]]
    );
    assert_eq!(
        legal_cards(
            Gamemode::Wenz(None),
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Unter
            })
        ),
        hand
    );
}

#[test]
fn wenz_and_geier_face_cards_follow_their_mode_specific_suit() {
    let hand = [
        Card {
            suit: Eichel,
            rank: Ober,
        },
        Card {
            suit: Eichel,
            rank: Unter,
        },
        Card {
            suit: Gras,
            rank: Sieben,
        },
    ];
    let lead = Some(Card {
        suit: Eichel,
        rank: Ass,
    });
    assert_eq!(
        legal_cards(Gamemode::Wenz(None), &hand, lead),
        vec![hand[0]]
    );
    assert_eq!(
        legal_cards(Gamemode::Geier(None), &hand, lead),
        vec![hand[1]]
    );
}

#[test]
fn suit_variants_combine_the_trump_rank_with_the_selected_suit() {
    let hand = [
        Card {
            suit: Eichel,
            rank: Ober,
        },
        Card {
            suit: Eichel,
            rank: Unter,
        },
        Card {
            suit: Gras,
            rank: Sieben,
        },
        Card {
            suit: Gras,
            rank: Ober,
        },
        Card {
            suit: Gras,
            rank: Unter,
        },
        Card {
            suit: Herz,
            rank: Ass,
        },
    ];
    let lead = Some(Card {
        suit: Gras,
        rank: Ass,
    });
    assert_eq!(
        legal_cards(Gamemode::Wenz(Some(Gras)), &hand, lead),
        vec![hand[1], hand[2], hand[3], hand[4]]
    );
    assert_eq!(
        legal_cards(Gamemode::Geier(Some(Gras)), &hand, lead),
        vec![hand[0], hand[2], hand[3], hand[4]]
    );
    assert_eq!(
        legal_cards(
            Gamemode::Wenz(Some(Gras)),
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Ass
            })
        ),
        vec![hand[0]]
    );
    assert_eq!(
        legal_cards(
            Gamemode::Geier(Some(Gras)),
            &hand,
            Some(Card {
                suit: Eichel,
                rank: Ass
            })
        ),
        vec![hand[1]]
    );
}

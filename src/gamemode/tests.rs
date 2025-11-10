use super::*;
use crate::deck::{Card, Suit, Rank};

fn card(suit: Suit, rank: Rank) -> Card {
    Card { suit, rank }
}

#[test]
fn winner_test_1() {
    let c1 = card(Suit::Herz, Rank::Ober);
    let c2 = card(Suit::Gras, Rank::Ober);
    let c3 = card(Suit::Schell, Rank::Ass);
    let c4 = card(Suit::Gras, Rank::Koenig);

    let winner = Gamemode::Sauspiel(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c1);

    let winner = Gamemode::Wenz(Some(Suit::Herz)).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c1);

    let winner = Gamemode::Wenz(Some(Suit::Gras)).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c4);

    let winner = Gamemode::Wenz(Some(Suit::Schell)).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    let winner = Gamemode::Geier(Some(Suit::Schell)).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    // Extra: Solo and Herz-trump modes behave consistently with O/U > suit trumps
    let winner = Gamemode::Solo(Suit::Gras).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    let winner = Gamemode::Bettel.winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    let winner = Gamemode::Ramsch.winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);
}

#[test]
fn sauspiel_trump_hierarchy() {
    // In Sauspiel, trump is always Herz; Obers > Unters > Herz-suit trumps
    let c1 = card(Suit::Eichel, Rank::Neun);     // led suit, non-trump
    let c2 = card(Suit::Gras, Rank::Ober);       // trump (Ober)
    let c3 = card(Suit::Herz, Rank::Ass);        // trump (trump suit)
    let c4 = card(Suit::Schell, Rank::Unter);    // trump (Unter)

    let winner = Gamemode::Sauspiel(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    // More checks on the same trick:
    // Wenz: only Unters are trump -> Unter wins
    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c4);

    // Geier: only Obers are trump -> Ober wins
    let winner = Gamemode::Geier(None).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    // Solo (any suit): O/U outrank suit trumps -> Ober wins
    let winner = Gamemode::Solo(Suit::Schell).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    // Herz-trump modes equivalent
    let winner = Gamemode::Bettel.winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);
    let winner = Gamemode::Ramsch.winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);
}

#[test]
fn sauspiel_ober_suit_precedence() {
    // Among Obers: Eichel > Gras > Herz > Schell
    let c1 = card(Suit::Gras, Rank::Koenig);     // led
    let c2 = card(Suit::Eichel, Rank::Ober);     // highest Ober
    let c3 = card(Suit::Herz, Rank::Ober);       // lower Ober
    let c4 = card(Suit::Schell, Rank::Unter);    // trump but below any Ober

    let winner = Gamemode::Sauspiel(Suit::Gras).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    // More checks:
    let winner = Gamemode::Solo(Suit::Schell).winning_card([&c1, &c2, &c3, &c4]); // O/U trump
    assert_eq!(winner, &c2);

    let winner = Gamemode::Geier(None).winning_card([&c1, &c2, &c3, &c4]); // Obers trump
    assert_eq!(winner, &c2);

    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]); // only Unter trump
    assert_eq!(winner, &c4);

    let winner = Gamemode::Bettel.winning_card([&c1, &c2, &c3, &c4]); // Herz-trump
    assert_eq!(winner, &c2);
}

#[test]
fn sauspiel_no_trump_led_suit_highest() {
    // No Obers/Unters and no Herz cards: highest of led suit wins (A > 10 > K > 9 > 8 > 7)
    let c1 = card(Suit::Eichel, Rank::Koenig);   // led suit
    let c2 = card(Suit::Gras, Rank::Ass);
    let c3 = card(Suit::Eichel, Rank::Zehn);     // higher than König
    let c4 = card(Suit::Schell, Rank::Neun);

    let winner = Gamemode::Sauspiel(Suit::Schell).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    // More checks:
    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]); // no Unters
    assert_eq!(winner, &c3);

    let winner = Gamemode::Solo(Suit::Gras).winning_card([&c1, &c2, &c3, &c4]); // Gras suit trump
    assert_eq!(winner, &c2);

    let winner = Gamemode::Solo(Suit::Schell).winning_card([&c1, &c2, &c3, &c4]); // Schell suit trump
    assert_eq!(winner, &c4);

    let winner = Gamemode::Solo(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // both Eichel trumps; A>10>K...
    assert_eq!(winner, &c3);

    let winner = Gamemode::Geier(Some(Suit::Schell)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Schell
    assert_eq!(winner, &c4);
}

#[test]
fn solo_suit_trumps_only_internal_order() {
    // In Solo, chosen suit is trump plus all Obers/Unters; with only suit trumps present, A > 10 > K > 9 > 8 > 7
    let c1 = card(Suit::Schell, Rank::Zehn);     // trump suit
    let c2 = card(Suit::Gras, Rank::Koenig);
    let c3 = card(Suit::Schell, Rank::Ass);      // highest among suit trumps
    let c4 = card(Suit::Eichel, Rank::Neun);

    let winner = Gamemode::Solo(Suit::Schell).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    // More checks:
    let winner = Gamemode::Sauspiel(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // no O/U, Herz not present -> follow suit
    assert_eq!(winner, &c3);

    let winner = Gamemode::Solo(Suit::Gras).winning_card([&c1, &c2, &c3, &c4]); // only Gras becomes trump
    assert_eq!(winner, &c2);

    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]); // no Unters -> follow suit
    assert_eq!(winner, &c3);

    let winner = Gamemode::Wenz(Some(Suit::Schell)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Schell
    assert_eq!(winner, &c3);
}

#[test]
fn wenz_unter_trumps_over_optional_suit_trump() {
    // In Wenz with extra suit trump, Unters outrank any suit trumps
    let c1 = card(Suit::Eichel, Rank::Ass);      // led
    let c2 = card(Suit::Gras, Rank::Koenig);     // trump by suit (Gras) if chosen
    let c3 = card(Suit::Schell, Rank::Unter);    // trump by Unter (beats suit trumps)
    let c4 = card(Suit::Gras, Rank::Ass);        // trump by suit (Gras) if chosen

    let winner = Gamemode::Wenz(Some(Suit::Gras)).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    // More checks:
    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]); // only Unter trump
    assert_eq!(winner, &c3);

    let winner = Gamemode::Geier(None).winning_card([&c1, &c2, &c3, &c4]); // no Obers -> follow suit
    assert_eq!(winner, &c1);

    let winner = Gamemode::Geier(Some(Suit::Gras)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Gras
    assert_eq!(winner, &c4);

    let winner = Gamemode::Sauspiel(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // O/U trump -> Unter wins
    assert_eq!(winner, &c3);
}

#[test]
fn wenz_unter_precedence_between_suits() {
    // Unter precedence: Eichel > Gras > Herz > Schell
    let c1 = card(Suit::Herz, Rank::Neun);       // led
    let c2 = card(Suit::Gras, Rank::Unter);
    let c3 = card(Suit::Eichel, Rank::Unter);    // highest Unter
    let c4 = card(Suit::Schell, Rank::Unter);

    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    // More checks:
    let winner = Gamemode::Wenz(Some(Suit::Gras)).winning_card([&c1, &c2, &c3, &c4]); // Unters still outrank suit trumps
    assert_eq!(winner, &c3);

    let winner = Gamemode::Geier(None).winning_card([&c1, &c2, &c3, &c4]); // no Obers -> follow suit
    assert_eq!(winner, &c1);

    let winner = Gamemode::Geier(Some(Suit::Gras)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Gras
    assert_eq!(winner, &c2);

    let winner = Gamemode::Sauspiel(Suit::Schell).winning_card([&c1, &c2, &c3, &c4]); // O/U trump -> highest Unter by suit
    assert_eq!(winner, &c3);
}

#[test]
fn wenz_no_trump_led_suit_highest() {
    // No Unters and no optional suit trumps: highest of led suit wins
    let c1 = card(Suit::Eichel, Rank::Koenig);   // led suit
    let c2 = card(Suit::Gras, Rank::Ass);
    let c3 = card(Suit::Eichel, Rank::Zehn);     // higher than König
    let c4 = card(Suit::Schell, Rank::Neun);

    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    // More checks:
    let winner = Gamemode::Wenz(Some(Suit::Gras)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Gras
    assert_eq!(winner, &c2);

    let winner = Gamemode::Geier(None).winning_card([&c1, &c2, &c3, &c4]); // no Obers -> follow suit
    assert_eq!(winner, &c3);

    let winner = Gamemode::Geier(Some(Suit::Schell)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Schell
    assert_eq!(winner, &c4);

    let winner = Gamemode::Solo(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // Eichel suit trump
    assert_eq!(winner, &c3);
}

#[test]
fn geier_ober_trumps_over_optional_suit_trump() {
    // In Geier with extra suit trump, Obers outrank any suit trumps
    let c1 = card(Suit::Gras, Rank::Ass);        // led
    let c2 = card(Suit::Schell, Rank::Koenig);   // trump by suit (optional)
    let c3 = card(Suit::Eichel, Rank::Ober);     // trump by Ober (beats suit trumps)
    let c4 = card(Suit::Schell, Rank::Ass);      // trump by suit (optional)

    let winner = Gamemode::Geier(Some(Suit::Schell)).winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    // More checks:
    let winner = Gamemode::Geier(None).winning_card([&c1, &c2, &c3, &c4]); // Obers trump
    assert_eq!(winner, &c3);

    let winner = Gamemode::Wenz(None).winning_card([&c1, &c2, &c3, &c4]); // no Unters -> follow suit
    assert_eq!(winner, &c1);

    let winner = Gamemode::Wenz(Some(Suit::Schell)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Schell
    assert_eq!(winner, &c4);

    let winner = Gamemode::Sauspiel(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // O/U trump -> Ober wins
    assert_eq!(winner, &c3);
}

#[test]
fn bettel_behaves_like_herz_trump() {
    // Current implementation treats Bettel like Herz-trump
    let c1 = card(Suit::Gras, Rank::Ass);        // led
    let c2 = card(Suit::Herz, Rank::Neun);       // trump by Herz suit
    let c3 = card(Suit::Schell, Rank::Ass);
    let c4 = card(Suit::Eichel, Rank::Koenig);

    let winner = Gamemode::Bettel.winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c2);

    // More checks:
    let winner = Gamemode::Ramsch.winning_card([&c1, &c2, &c3, &c4]); // same as Bettel currently
    assert_eq!(winner, &c2);

    let winner = Gamemode::Sauspiel(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // Herz trump
    assert_eq!(winner, &c2);

    let winner = Gamemode::Solo(Suit::Herz).winning_card([&c1, &c2, &c3, &c4]); // Herz trump in Solo
    assert_eq!(winner, &c2);

    let winner = Gamemode::Solo(Suit::Gras).winning_card([&c1, &c2, &c3, &c4]); // Gras trump; no O/U
    assert_eq!(winner, &c1);

    let winner = Gamemode::Wenz(Some(Suit::Herz)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Herz
    assert_eq!(winner, &c2);

    let winner = Gamemode::Geier(Some(Suit::Herz)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Herz
    assert_eq!(winner, &c2);
}

#[test]
fn ramsch_behaves_like_herz_trump() {
    // Current implementation treats Ramsch like Herz-trump
    let c1 = card(Suit::Eichel, Rank::Ass);      // led
    let c2 = card(Suit::Schell, Rank::Koenig);
    let c3 = card(Suit::Herz, Rank::Zehn);       // trump by Herz suit
    let c4 = card(Suit::Gras, Rank::Neun);

    let winner = Gamemode::Ramsch.winning_card([&c1, &c2, &c3, &c4]);
    assert_eq!(winner, &c3);

    // More checks:
    let winner = Gamemode::Bettel.winning_card([&c1, &c2, &c3, &c4]); // same as Ramsch currently
    assert_eq!(winner, &c3);

    let winner = Gamemode::Sauspiel(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // Herz trump
    assert_eq!(winner, &c3);

    let winner = Gamemode::Solo(Suit::Herz).winning_card([&c1, &c2, &c3, &c4]); // Herz trump in Solo
    assert_eq!(winner, &c3);

    let winner = Gamemode::Solo(Suit::Eichel).winning_card([&c1, &c2, &c3, &c4]); // Eichel trump; no O/U
    assert_eq!(winner, &c1);

    let winner = Gamemode::Wenz(Some(Suit::Herz)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Herz
    assert_eq!(winner, &c3);

    let winner = Gamemode::Geier(Some(Suit::Herz)).winning_card([&c1, &c2, &c3, &c4]); // suit trump Herz
    assert_eq!(winner, &c3);
}
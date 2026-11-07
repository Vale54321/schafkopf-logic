use crate::{
    deck::{Card, Deck},
    game::{PlayedCard, PlayerId},
    gamemode::Gamemode,
    observation::Observation,
};

pub(super) fn active_trick(view: &Observation) -> &[PlayedCard] {
    if view.trick.len() == 4 {
        &[]
    } else {
        &view.trick
    }
}

pub(super) fn category(mode: Gamemode, card: Card) -> usize {
    if mode.is_trump(&card) {
        0
    } else {
        1 + card.suit as usize
    }
}

/// Derived exclusively from public plays. A failure to follow a category proves
/// that seat has none left. Categories distinguish trumps from printed suits.
pub(super) struct Knowledge {
    pub unseen: Vec<Card>,
    pub voids: [u8; 4],
}

impl Knowledge {
    pub fn new(view: &Observation) -> Self {
        let mut seen = Vec::new();
        let mut voids = [0; 4];
        for plays in view
            .history
            .iter()
            .map(|trick| trick.plays.as_slice())
            .chain([view.trick.as_slice()])
        {
            if let Some(lead) = plays.first() {
                let led = category(view.config.mode, lead.card);
                for play in plays {
                    if !seen.contains(&play.card) {
                        seen.push(play.card);
                    }
                    if category(view.config.mode, play.card) != led {
                        voids[play.player.index()] |= 1 << led;
                    }
                }
            }
        }
        let unseen = Deck::new()
            .iter()
            .copied()
            .filter(|card| !view.hand.contains(card) && !seen.contains(card))
            .collect();
        Self { unseen, voids }
    }

    /// Estimate the chance a later seat can (or must) overtake a partial trick.
    /// Uniform unseen-card estimates are conditioned on publicly known voids;
    /// they do not inspect the actual remaining hands.
    pub fn overtake(
        &self,
        view: &Observation,
        plays: &[PlayedCard],
        seat: PlayerId,
        forced: bool,
    ) -> f64 {
        let mode = view.config.mode;
        let lead = plays[0].card;
        let pool: Vec<_> = self
            .unseen
            .iter()
            .copied()
            .filter(|card| self.voids[seat.index()] & (1 << category(mode, *card)) == 0)
            .collect();
        let n = pool.len();
        let count = view.hand_sizes[seat.index()].min(n);
        if count == 0 {
            return 0.0;
        }
        let mut cards: Vec<_> = plays.iter().map(|play| play.card).collect();
        let mut following = 0;
        let mut winning_following = 0;
        let mut winning_off = 0;
        for card in &pool {
            let follows = category(mode, *card) == category(mode, lead);
            if follows {
                following += 1;
            }
            cards.push(*card);
            let wins = mode.winning_index(&cards) == Some(cards.len() - 1);
            cards.pop();
            if wins {
                if follows {
                    winning_following += 1;
                } else {
                    winning_off += 1;
                }
            }
        }
        if forced {
            // With following cards, every possible follower must overtake.
            // When void, every card in the hand must overtake.
            (none_of(n, following - winning_following, count) - none_of(n, following, count)
                + none_of(n, n - winning_off, count))
            .clamp(0.0, 1.0)
        } else {
            // Higher followers, or (when void) a winning off-category trump.
            (1.0 - none_of(n, winning_following, count) + none_of(n, following, count)
                - none_of(n, following + winning_off, count))
            .clamp(0.0, 1.0)
        }
    }
}

/// Hypergeometric probability that a hand contains none of a selected subset.
fn none_of(population: usize, selected: usize, draws: usize) -> f64 {
    if draws > population.saturating_sub(selected) {
        return 0.0;
    }
    (0..draws).fold(1.0, |p, i| {
        p * (population - selected - i) as f64 / (population - i) as f64
    })
}

pub(super) fn remaining_seats(view: &Observation) -> Vec<PlayerId> {
    (1..=3 - active_trick(view).len())
        .map(|offset| PlayerId::ALL[(view.player.index() + offset) % 4])
        .collect()
}

pub(super) fn ally(view: &Observation, other: PlayerId) -> Option<bool> {
    if other == view.player {
        return Some(true);
    }
    match view.config.mode {
        Gamemode::Ramsch => Some(false),
        Gamemode::Sauspiel(_) => {
            if let Some(partner) = view.partner {
                let declaring = |seat| seat == view.config.declarer || seat == partner;
                Some(declaring(view.player) == declaring(other))
            } else if other == view.config.declarer {
                Some(false)
            } else {
                None
            }
        }
        _ => Some((view.player == view.config.declarer) == (other == view.config.declarer)),
    }
}

pub(super) fn ally_chance(view: &Observation, other: PlayerId) -> f64 {
    match ally(view, other) {
        Some(true) => 1.0,
        Some(false) => 0.0,
        None if view.player == view.config.declarer => 1.0 / 3.0,
        None => 0.5,
    }
}

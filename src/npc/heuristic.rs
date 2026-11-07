use super::{
    Decision, DecisionReason, NpcStrategy, StrategyInfo,
    knowledge::{self, Knowledge},
};
use crate::{
    deck::{Card, Rank},
    game::{Action, PlayedCard, PlayerId},
    gamemode::Gamemode,
    observation::Observation,
};

/// Deterministic tactics with public card memory, no search into hidden hands.
#[derive(Default)]
pub struct HeuristicNpc;

impl NpcStrategy for HeuristicNpc {
    fn info(&self) -> StrategyInfo {
        StrategyInfo {
            name: "heuristic".into(),
            version: 1,
        }
    }
    fn choose_action(&mut self, view: &Observation) -> Option<Decision> {
        if view.next_player != Some(view.player) {
            return None;
        }
        let knowledge = Knowledge::new(view);
        view.legal_cards()
            .map(|card| {
                let (score, reason) = evaluate(view, &knowledge, card);
                (score, card, reason)
            })
            .max_by(|(score_a, card_a, _), (score_b, card_b, _)| {
                score_a
                    .total_cmp(score_b)
                    .then_with(|| (card_b.suit, card_b.rank).cmp(&(card_a.suit, card_a.rank)))
            })
            .map(|(_, card, reason)| Decision {
                action: Action::PlayCard(card),
                reason,
            })
    }
}

fn control(mode: Gamemode, card: Card) -> f64 {
    if mode.is_trump(&card) {
        40.0 + f64::from(mode.trump_strength(&card)) / 4.0
    } else {
        match card.rank {
            Rank::Ass => 36.0,
            Rank::Zehn => 28.0,
            Rank::Koenig => 20.0,
            Rank::Ober => 16.0,
            Rank::Unter => 12.0,
            Rank::Neun => 6.0,
            Rank::Acht => 3.0,
            Rank::Sieben => 0.0,
        }
    }
}

fn evaluate(view: &Observation, known: &Knowledge, card: Card) -> (f64, DecisionReason) {
    let mode = view.config.mode;
    let before = knowledge::active_trick(view);
    let mut plays = before.to_vec();
    plays.push(PlayedCard {
        player: view.player,
        card,
    });
    let cards: Vec<_> = plays.iter().map(|play| play.card).collect();
    let winner = plays[mode.winning_index(&cards).unwrap()].player;
    let remaining = knowledge::remaining_seats(view);
    let hold = remaining.iter().fold(1.0, |p, seat| {
        p * (1.0 - known.overtake(view, &plays, *seat, false))
    });
    let points: f64 = before
        .iter()
        .map(|play| f64::from(play.card.rank.points()))
        .sum();
    let card_points = f64::from(card.rank.points());
    let cost = control(mode, card);

    if mode == Gamemode::Bettel {
        if view.player == view.config.declarer {
            let forced_hold = remaining.iter().fold(1.0, |p, seat| {
                p * (1.0 - known.overtake(view, &plays, *seat, true))
            });
            let own_win = if winner == view.player {
                forced_hold
            } else {
                0.0
            };
            // First avoid the trick, then shed the highest dangerous loser.
            return (
                -250.0 * own_win + cost * 0.18,
                DecisionReason::AvoidBettelTrick,
            );
        }
        let force = if winner == view.config.declarer {
            remaining.iter().fold(1.0, |p, seat| {
                p * (1.0 - known.overtake(view, &plays, *seat, true))
            })
        } else if remaining.contains(&view.config.declarer) {
            known.overtake(view, &plays, view.config.declarer, true)
        } else {
            0.0
        };
        return (
            250.0 * force - cost * 0.03,
            DecisionReason::ForceBettelTrick,
        );
    }

    if mode == Gamemode::Ramsch {
        // Opponents also avoid taking points: do not count on a voluntary rescue.
        let forced_hold = remaining.iter().fold(1.0, |p, seat| {
            p * (1.0 - known.overtake(view, &plays, *seat, true))
        });
        let own_win = if winner == view.player {
            forced_hold
        } else {
            0.0
        };
        let chase = view.completed_tricks >= 4
            && view.tricks_won[view.player.index()] == view.completed_tricks;
        let stop = view.completed_tricks >= 5
            && PlayerId::ALL.into_iter().any(|seat| {
                seat != view.player && view.tricks_won[seat.index()] == view.completed_tricks
            });
        let mut score = -5.0 * own_win * (points + card_points + 5.0)
            + (1.0 - own_win) * (card_points * 1.5 + cost * 0.08);
        let reason = if chase {
            score += 350.0 * own_win;
            DecisionReason::ChaseDurchmarsch
        } else if stop {
            score += 400.0 * own_win;
            DecisionReason::StopDurchmarsch
        } else {
            DecisionReason::AvoidRamschPoints
        };
        return (score, reason);
    }

    let mut team_win = knowledge::ally_chance(view, winner);
    for seat in &remaining {
        let overtakes = known.overtake(view, &plays, *seat, false);
        team_win = team_win * (1.0 - overtakes) + knowledge::ally_chance(view, *seat) * overtakes;
    }
    // Capturing opponents' points and retaining control matters. Own points
    // are worth feeding to a teammate, but do not justify wasting a stronger
    // winning card when a cheaper one takes the same trick.
    let feed = if winner != view.player {
        team_win * card_points
    } else {
        0.0
    };
    let mut score = (2.0 * team_win - 1.0) * (points + 8.0) + feed
        - (1.0 - team_win) * card_points * 1.2
        - cost * 0.15;
    let mut reason = if knowledge::ally(view, winner) == Some(true) && winner != view.player {
        DecisionReason::SupportTeammate
    } else if winner == view.player {
        DecisionReason::WinTrick
    } else {
        DecisionReason::ConserveCards
    };

    if before.is_empty() {
        if view.player == view.config.declarer
            && mode.is_trump(&card)
            && view.hand.iter().filter(|card| mode.is_trump(card)).count() >= 5
        {
            score += 18.0 * hold;
            reason = DecisionReason::LeadTrump;
        } else if !mode.is_trump(&card) && card.rank == Rank::Ass {
            reason = DecisionReason::LeadSideAce;
        }
        if let Gamemode::Sauspiel(suit) = mode {
            if view.player == view.config.declarer
                && view.partner.is_none()
                && card.suit == suit
                && !mode.is_trump(&card)
            {
                score += 12.0;
                reason = DecisionReason::SearchCalledAce;
            }
        }
    }
    (score, reason)
}

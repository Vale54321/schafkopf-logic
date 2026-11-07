use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{deck::Card, gamemode::Gamemode};

/// A seat at a four-player table. IDs are zero-based and stable for a game.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
pub struct PlayerId(u8);

impl TryFrom<u8> for PlayerId {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value as usize).ok_or("seat must be in 0..4")
    }
}
impl From<PlayerId> for u8 {
    fn from(player: PlayerId) -> Self {
        player.0
    }
}

impl PlayerId {
    pub const ALL: [Self; 4] = [Self(0), Self(1), Self(2), Self(3)];

    pub const fn new(index: usize) -> Option<Self> {
        if index < 4 {
            Some(Self(index as u8))
        } else {
            None
        }
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn next(self) -> Self {
        Self((self.0 + 1) % 4)
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Player {}", self.index() + 1)
    }
}

/// Explicit initial conditions; shuffling happens before constructing the game.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameConfig {
    pub mode: Gamemode,
    pub first_player: PlayerId,
    /// Independent of the opening seat. Ignored for Ramsch.
    pub declarer: PlayerId,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Action {
    PlayCard(Card),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayedCard {
    pub player: PlayerId,
    pub card: Card,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedTrick {
    /// Plays in chronological order, starting with the leader.
    pub plays: [PlayedCard; 4],
    pub winner: PlayerId,
}

impl CompletedTrick {
    pub fn points(&self) -> u16 {
        self.plays
            .iter()
            .map(|play| u16::from(play.card.rank.points()))
            .sum()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GameResult {
    pub winners: Vec<PlayerId>,
    pub points: [u16; 4],
    pub tricks_won: [u8; 4],
    pub reason: ResultReason,
}

/// Totals for an outcome group. In Ramsch, grouped players are not partners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeSide {
    pub players: Vec<PlayerId>,
    pub points: u16,
    pub tricks: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeSummary {
    Decided {
        winners: OutcomeSide,
        losers: OutcomeSide,
    },
    Draw(OutcomeSide),
}

impl GameResult {
    /// Uses the authoritative winners; points alone do not decide Bettel or Durchmarsch.
    pub fn summary(&self) -> OutcomeSummary {
        let side = |players: Vec<PlayerId>| OutcomeSide {
            points: players.iter().map(|p| self.points[p.index()]).sum(),
            tricks: players.iter().map(|p| self.tricks_won[p.index()]).sum(),
            players,
        };
        if self.winners.is_empty() {
            OutcomeSummary::Draw(side(PlayerId::ALL.to_vec()))
        } else {
            OutcomeSummary::Decided {
                winners: side(PlayerId::ALL.into_iter().filter(|p| self.won(*p)).collect()),
                losers: side(
                    PlayerId::ALL
                        .into_iter()
                        .filter(|p| !self.won(*p))
                        .collect(),
                ),
            }
        }
    }

    pub fn won(&self, player: PlayerId) -> bool {
        self.winners.contains(&player)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ResultReason {
    Points {
        declarer: PlayerId,
        partner: Option<PlayerId>,
        team_points: u16,
    },
    Bettel {
        declarer: PlayerId,
        tricks_taken: u8,
    },
    Ramsch {
        highest_points: u16,
    },
    Durchmarsch {
        player: PlayerId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SauspielError {
    HeartsCannotBeCalled,
    DeclarerHoldsCalledAce,
    DeclarerLacksCalledSuit,
}

/// Completed tricks remain visible until the next leader plays a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    PlayingTrick,
    TrickComplete,
    Finished,
}

/// Public outcomes of accepted actions. Events never contain private hands.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum GameEvent {
    CardPlayed(PlayedCard),
    TrickCompleted(CompletedTrick),
    GameFinished(GameResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    InvalidSauspiel(SauspielError),
    InvalidDeck {
        cards_remaining: usize,
    },
    WrongTurn {
        expected: PlayerId,
        actual: PlayerId,
    },
    CardNotInHand {
        player: PlayerId,
        card: Card,
    },
    MustFollowSuit {
        player: PlayerId,
        suit: crate::deck::Suit,
    },
    MustFollowTrump {
        player: PlayerId,
    },
    CalledAceRestriction {
        player: PlayerId,
    },
    GameAlreadyFinished,
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSauspiel(reason) => write!(f, "invalid Sauspiel declaration: {reason:?}"),
            Self::InvalidDeck { cards_remaining } => {
                write!(
                    f,
                    "expected a full 32-card deck, got {cards_remaining} cards"
                )
            }
            Self::WrongTurn { expected, actual } => {
                write!(f, "it is {expected}'s turn, not {actual}'s")
            }
            Self::CardNotInHand { player, card } => {
                write!(f, "{player} does not hold {card}")
            }
            Self::MustFollowSuit { player, suit } => write!(f, "{player} must follow {suit}"),
            Self::MustFollowTrump { player } => write!(f, "{player} must play a trump card"),
            Self::CalledAceRestriction { player } => {
                write!(f, "{player} must respect the called-ace rules")
            }
            Self::GameAlreadyFinished => write!(f, "this game is already finished"),
        }
    }
}

impl std::error::Error for GameError {}

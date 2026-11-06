use std::fmt;

use crate::{deck::Card, gamemode::Gamemode};

/// A seat at a four-player table. IDs are zero-based and stable for a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerId(u8);

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameConfig {
    pub mode: Gamemode,
    pub first_player: PlayerId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PlayCard(Card),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayedCard {
    pub player: PlayerId,
    pub card: Card,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedTrick {
    /// Plays in chronological order, starting with the leader.
    pub plays: [PlayedCard; 4],
    pub winner: PlayerId,
}

/// The demo stops after one trick; a complete match will extend these phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    PlayingTrick,
    TrickComplete,
}

/// Public outcomes of accepted actions. Events never contain private hands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameEvent {
    CardPlayed(PlayedCard),
    TrickCompleted(CompletedTrick),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
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
    TrickAlreadyComplete,
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::TrickAlreadyComplete => write!(f, "this one-trick game is complete"),
        }
    }
}

impl std::error::Error for GameError {}

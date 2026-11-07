//! Frontend-independent declaration round, match lifecycle, and complete journal.
//! Each seat declares once in order. Higher-ranked bids replace the current bid;
//! equal ranks stay with the earlier seat. Four passes produce Ramsch.

use crate::{
    deck::{Card, Deck},
    game::{Action, Game, GameConfig, GameError, GameEvent, GameResult, PlayerId},
    gamemode::Gamemode,
    observation::Observation,
    rating::{self, HandRating},
};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const RECORD_VERSION: u32 = 2;
pub const FORCED_RECORD_VERSION: u32 = 3;
pub const RULES: &str = "single_declaration_v1;solo>pure_wenz_geier>suit_wenz_geier>bettel>sauspiel;equal_rank_earlier_seat;all_pass_ramsch;bettel_herz_trumps;ramsch_highest_loses_all8_durchmarsch";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedDeclaration {
    pub observation: DeclarationObservation,
    pub choice: Option<Gamemode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundPhase {
    Declaring,
    Playing,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    pub player: PlayerId,
    /// None means pass. Ramsch cannot be announced.
    pub mode: Option<Gamemode>,
    pub score: Option<u8>,
}

impl Declaration {
    // The diagnostic score is computed from a private hand. Only the announced
    // mode/pass belongs in other players' observations.
    fn public(&self) -> Self {
        Self {
            score: None,
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedPlay {
    pub player: PlayerId,
    pub action: Action,
    pub events: Vec<GameEvent>,
}

/// Explicit override of bidding priority. Only the current bidder may force a
/// hand-legal contract, before card play. Unasked players are not recorded as passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForcedContract {
    pub player: PlayerId,
    pub mode: Gamemode,
}

/// Complete trusted journal, including private hands. Never send this to an
/// opponent during a game; use observation() for player-facing data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundRecord {
    pub schema_version: u32,
    pub rules: String,
    pub rating_version: u32,
    pub first_player: PlayerId,
    pub deal_order: Vec<Card>,
    pub initial_hands: [Vec<Card>; 4],
    pub ratings: [Vec<HandRating>; 4],
    pub declarations: Vec<Declaration>,
    #[serde(default)]
    pub declaration_decisions: Vec<RecordedDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced_contract: Option<ForcedContract>,
    pub config: Option<GameConfig>,
    pub plays: Vec<RecordedPlay>,
    pub result: Option<GameResult>,
}

/// Private hand and public declaration state for one authenticated seat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationObservation {
    pub player: PlayerId,
    pub hand: Vec<Card>,
    pub next_bidder: Option<PlayerId>,
    pub first_player: PlayerId,
    pub legal_modes: Vec<Gamemode>,
    pub leading_bid: Option<Declaration>,
    pub declarations: Vec<Declaration>,
}

#[derive(Clone)]
pub struct Round {
    deck: Deck,
    game: Game,
    next_bidder: Option<PlayerId>,
    leading_bid: Option<Declaration>,
    record: RoundRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundError {
    Engine(GameError),
    NotDeclaring,
    WrongBidder { expected: PlayerId },
    MustOutbid,
    InvalidDeclaration,
    WaitingForDeclarations,
}

impl fmt::Display for RoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => error.fmt(f),
            Self::NotDeclaring => write!(f, "Declarations are closed for this game."),
            Self::WrongBidder { expected } => write!(f, "Waiting for {expected}'s declaration."),
            Self::MustOutbid => write!(f, "Choose a higher-ranked game or pass."),
            Self::InvalidDeclaration => write!(f, "This game cannot be declared with your hand."),
            Self::WaitingForDeclarations => {
                write!(f, "Wait until all four players have declared or passed.")
            }
        }
    }
}
impl std::error::Error for RoundError {}
impl From<GameError> for RoundError {
    fn from(error: GameError) -> Self {
        Self::Engine(error)
    }
}

impl Round {
    pub fn new(deck: Deck, first_player: PlayerId) -> Result<Self, GameError> {
        let game = Game::new(
            GameConfig {
                mode: Gamemode::Ramsch,
                first_player,
                declarer: first_player,
            },
            deck.clone(),
        )?;
        let hands = std::array::from_fn(|i| game.observation(PlayerId::ALL[i]).hand);
        let record = RoundRecord {
            schema_version: RECORD_VERSION,
            rules: RULES.into(),
            rating_version: 1,
            first_player,
            deal_order: deck.deal_order(),
            ratings: std::array::from_fn(|i| rating::ratings(&hands[i])),
            initial_hands: hands,
            declarations: Vec::new(),
            declaration_decisions: Vec::new(),
            forced_contract: None,
            config: None,
            plays: Vec::new(),
            result: None,
        };
        Ok(Self {
            deck,
            game,
            next_bidder: Some(first_player),
            leading_bid: None,
            record,
        })
    }

    pub fn phase(&self) -> RoundPhase {
        if self.next_bidder.is_some() {
            RoundPhase::Declaring
        } else if self.game.result().is_some() {
            RoundPhase::Finished
        } else {
            RoundPhase::Playing
        }
    }
    pub fn next_bidder(&self) -> Option<PlayerId> {
        self.next_bidder
    }
    pub fn leading_bid(&self) -> Option<&Declaration> {
        self.leading_bid.as_ref()
    }
    pub fn declarations(&self) -> &[Declaration] {
        &self.record.declarations
    }
    pub fn ratings(&self, player: PlayerId) -> &[HandRating] {
        &self.record.ratings[player.index()]
    }
    pub fn record(&self) -> &RoundRecord {
        &self.record
    }

    pub fn observation(&self, player: PlayerId) -> Observation {
        let mut view = self.game.observation(player);
        view.declarations = self
            .record
            .declarations
            .iter()
            .map(Declaration::public)
            .collect();
        if self.phase() == RoundPhase::Declaring {
            view.next_player = None;
            view.legal_actions.clear();
        }
        view
    }

    pub fn declaration_observation(&self, player: PlayerId) -> DeclarationObservation {
        DeclarationObservation {
            player,
            hand: self.game.observation(player).hand,
            next_bidder: self.next_bidder,
            first_player: self.record.first_player,
            legal_modes: self.legal_declarations(player),
            leading_bid: self.leading_bid.as_ref().map(Declaration::public),
            declarations: self
                .record
                .declarations
                .iter()
                .map(Declaration::public)
                .collect(),
        }
    }

    pub fn legal_declarations(&self, player: PlayerId) -> Vec<Gamemode> {
        if self.next_bidder != Some(player) {
            return Vec::new();
        }
        self.ratings(player)
            .iter()
            .filter(|r| {
                r.eligible
                    && self.leading_bid.as_ref().is_none_or(|bid| {
                        rating::priority(r.mode) > rating::priority(bid.mode.unwrap())
                    })
            })
            .map(|r| r.mode)
            .collect()
    }

    pub fn declare(&mut self, player: PlayerId, mode: Option<Gamemode>) -> Result<(), RoundError> {
        let expected = self.next_bidder.ok_or(RoundError::NotDeclaring)?;
        if player != expected {
            return Err(RoundError::WrongBidder { expected });
        }
        if let Some(mode) = mode {
            if !rating::eligible(&self.game.observation(player).hand, mode) {
                return Err(RoundError::InvalidDeclaration);
            }
            if self
                .leading_bid
                .as_ref()
                .is_some_and(|bid| rating::priority(mode) <= rating::priority(bid.mode.unwrap()))
            {
                return Err(RoundError::MustOutbid);
            }
        }
        let recorded = RecordedDeclaration {
            observation: self.declaration_observation(player),
            choice: mode,
        };
        let declaration = Declaration {
            player,
            mode,
            score: mode.map(|mode| {
                self.ratings(player)
                    .iter()
                    .find(|r| r.mode == mode)
                    .unwrap()
                    .score
            }),
        };
        if mode.is_some() {
            self.leading_bid = Some(declaration.clone());
        }
        self.record.declaration_decisions.push(recorded);
        self.record.declarations.push(declaration);
        if self.record.declarations.len() == 4 {
            let config = GameConfig {
                mode: self
                    .leading_bid
                    .as_ref()
                    .and_then(|bid| bid.mode)
                    .unwrap_or(Gamemode::Ramsch),
                declarer: self
                    .leading_bid
                    .as_ref()
                    .map(|bid| bid.player)
                    .unwrap_or(self.record.first_player),
                first_player: self.record.first_player,
            };
            self.game = Game::new(config, self.deck.clone())?;
            self.record.config = Some(config);
            self.next_bidder = None;
        } else {
            self.next_bidder = Some(player.next());
        }
        Ok(())
    }

    /// Start this bidder's selected contract immediately, bypassing bid priority.
    /// Card rules, declaration eligibility, deal, and opening seat are preserved.
    pub fn force_game(&mut self, player: PlayerId, mode: Gamemode) -> Result<(), RoundError> {
        let expected = self.next_bidder.ok_or(RoundError::NotDeclaring)?;
        if player != expected {
            return Err(RoundError::WrongBidder { expected });
        }
        if !rating::eligible(&self.game.observation(player).hand, mode) {
            return Err(RoundError::InvalidDeclaration);
        }
        let config = GameConfig {
            mode,
            declarer: player,
            first_player: self.record.first_player,
        };
        let game = Game::new(config, self.deck.clone())?;
        let score = self
            .ratings(player)
            .iter()
            .find(|r| r.mode == mode)
            .map(|r| r.score);
        self.game = game;
        self.leading_bid = Some(Declaration {
            player,
            mode: Some(mode),
            score,
        });
        self.record.forced_contract = Some(ForcedContract { player, mode });
        self.record.schema_version = FORCED_RECORD_VERSION;
        self.record.config = Some(config);
        self.next_bidder = None;
        Ok(())
    }

    pub fn play(&mut self, player: PlayerId, action: Action) -> Result<Vec<GameEvent>, RoundError> {
        if self.phase() == RoundPhase::Declaring {
            return Err(RoundError::WaitingForDeclarations);
        }
        let events = self.game.apply(player, action)?;
        self.record.plays.push(RecordedPlay {
            player,
            action,
            events: events.clone(),
        });
        self.record.result = self.game.result().cloned();
        Ok(events)
    }
}

#[cfg(test)]
mod tests;

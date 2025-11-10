use std::io::{self, Write};
use crate::deck::Card;
use std::fmt;

#[derive(Debug)]
pub enum PlayerError {
    NoCards,
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlayerError::NoCards => write!(f, "no cards available to play"),
        }
    }
}

impl std::error::Error for PlayerError {}

pub struct PlayerBase {
    pub id: u32,
    pub name: String,
}

impl PlayerBase {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self { id, name: name.into() }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn play_card(&mut self, hand: &mut Vec<Card>) -> Result<Card, PlayerError> {
        hand.pop().ok_or(PlayerError::NoCards)
    }
}

pub trait PlayerBaseAccess {
    fn base(&self) -> &PlayerBase;
    fn base_mut(&mut self) -> &mut PlayerBase;

    fn id(&self) -> u32 {
        self.base().id()
    }

    fn name(&self) -> &str {
        self.base().name()
    }

    fn set_name(&mut self, name: impl Into<String>) {
        self.base_mut().set_name(name);
    }
}

pub trait ExternalPlayer: PlayerBaseAccess {
    fn play_card(&mut self, hand: &mut Vec<Card>) -> Result<Card, PlayerError> {
        self.base_mut().play_card(hand)
    }
}

pub trait InternalPlayer: PlayerBaseAccess {
    fn play_card_from_hand(&mut self) -> Result<Card, PlayerError>;
    fn receive_card(&mut self, card: Card);
    fn set_hand(&mut self, hand: Vec<Card>);
    fn hand(&self) -> &Vec<Card>;
}

pub struct HumanPlayer {
    pub base: PlayerBase,
    pub hand: Vec<Card>,
}

impl HumanPlayer {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            base: PlayerBase::new(id, name),
            hand: Vec::with_capacity(8),
        }
    }
}

impl PlayerBaseAccess for HumanPlayer {
    fn base(&self) -> &PlayerBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut PlayerBase {
        &mut self.base
    }
}

impl InternalPlayer for HumanPlayer {
    fn play_card_from_hand(&mut self) -> Result<Card, PlayerError> {
        if self.hand.is_empty() {
            return Err(PlayerError::NoCards);
        }

        println!("{}'s hand:", self.name());
        for (i, c) in self.hand.iter().enumerate() {
            println!("  {}: {}", i, c);
        }
        print!("Select card index to play: ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            if let Ok(idx) = input.trim().parse::<usize>() {
                if idx < self.hand.len() {
                    return Ok(self.hand.remove(idx));
                }
            }
        }

        // fallback: pop last
        self.hand.pop().ok_or(PlayerError::NoCards)
    }

    fn receive_card(&mut self, card: Card) {
        self.hand.push(card);
    }

    fn set_hand(&mut self, hand: Vec<Card>) {
        self.hand = hand;
    }

    fn hand(&self) -> &Vec<Card> {
        &self.hand
    }
}

pub struct NpcPlayer {
    pub base: PlayerBase,
    pub hand: Vec<Card>,
}

impl NpcPlayer {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self { base: PlayerBase::new(id, name), hand: Vec::with_capacity(8) }
    }
}

impl PlayerBaseAccess for NpcPlayer {
    fn base(&self) -> &PlayerBase { &self.base }
    fn base_mut(&mut self) -> &mut PlayerBase { &mut self.base }
}

impl InternalPlayer for NpcPlayer {
    fn play_card_from_hand(&mut self) -> Result<Card, PlayerError> {
        if self.hand.is_empty() {
            Err(PlayerError::NoCards)
        } else {
            Ok(self.hand.remove(0))
        }
    }

    fn receive_card(&mut self, card: Card) {
        self.hand.push(card);
    }

    fn set_hand(&mut self, hand: Vec<Card>) {
        self.hand = hand;
    }

    fn hand(&self) -> &Vec<Card> {
        &self.hand
    }
}
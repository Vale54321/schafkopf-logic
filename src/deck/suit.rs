use serde::{Deserialize, Serialize};
use std::fmt;
use strum_macros::EnumIter;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum Suit {
    Eichel,
    Gras,
    Herz,
    Schell,
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Suit::Eichel => "Eichel",
            Suit::Gras => "Gras",
            Suit::Herz => "Herz",
            Suit::Schell => "Schell",
        };
        write!(f, "{}", name)
    }
}

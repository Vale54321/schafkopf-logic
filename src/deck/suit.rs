use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter)]
pub enum Suit {
    Eichel,
    Gras,
    Herz,
    Schell,
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Suit::Eichel    => "Eichel",
            Suit::Gras      => "Gras",
            Suit::Herz      => "Herz",
            Suit::Schell    => "Schell",
        };
        write!(f, "{}", name)
    }
}
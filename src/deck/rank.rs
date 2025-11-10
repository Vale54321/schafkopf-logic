use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter)]
pub enum Rank {
    Ass,
    Zehn,
    Koenig,
    Ober,
    Unter,
    Neun,
    Acht,
    Sieben,
}

impl Rank {
    pub fn points(&self) -> u8 {
        match self {
            Rank::Ass       => 11,
            Rank::Zehn      => 10,
            Rank::Koenig    =>  4,
            Rank::Ober      =>  3,
            Rank::Unter     =>  3,
            _               =>  0
        }
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Rank::Ass       => "Ass",
            Rank::Zehn      => "Zehn",
            Rank::Koenig    => "König",
            Rank::Ober      => "Ober",
            Rank::Unter     => "Unter",
            Rank::Neun      => "Neun",
            Rank::Acht      => "Acht",
            Rank::Sieben    => "Sieben",
        };
        write!(f, "{}", name)
    }
}
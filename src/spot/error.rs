use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum SpotClientError {
    Price(String),
    Trading(String),
    Decimal(String),
}

impl Error for SpotClientError {}

impl Display for SpotClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Price(e) => write!(f, "{}", e),
            Self::Trading(e) => write!(f, "{}", e),
            Self::Decimal(e) => write!(f, "{} to decimal error", e),
        }
    }
}

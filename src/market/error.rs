#[derive(Debug)]
pub enum MarketClientError {
    Client(String),
    Decimal(String),
}

impl std::error::Error for MarketClientError {}

impl std::fmt::Display for MarketClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(e) => write!(f, "{}", e),
            Self::Decimal(e) => write!(f, "{} to decimal error", e),
        }
    }
}

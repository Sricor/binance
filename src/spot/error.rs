#[derive(Debug)]
pub enum SpotClientError {
    Trading(String),
    Strategy(String),
    Decimal,
}

impl std::error::Error for SpotClientError {}

impl std::fmt::Display for SpotClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trading(e) => write!(f, "{}", e),
            Self::Strategy(e) => write!(f, "{}", e),
            Self::Decimal => write!(f, "to decimal error"),
        }
    }
}

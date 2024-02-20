use binance::{api::Binance, market::Market};
use rust_decimal::prelude::FromPrimitive;

use super::error::MarketClientError;
use crate::{noun::*, strategy::PriceSignal};

type MarketClientResult<T> = Result<T, MarketClientError>;

pub struct MarketClient {
    client: Market,
}

impl MarketClient {
    pub fn new() -> Self {
        let client = Market::new(None, None);
        Self { client }
    }

    pub async fn price(&self, symbol: &Symbol) -> MarketClientResult<PriceSignal> {
        match self.client.get_price(symbol).await {
            Ok(v) => {
                if let Some(price) = Decimal::from_f64(v.price) {
                    let result = PriceSignal::new(price);

                    Ok(result)
                } else {
                    let result = MarketClientError::Decimal(v.price.to_string());

                    Err(result)
                }
            }
            Err(e) => Err(MarketClientError::Client(e.to_string())),
        }
    }
}

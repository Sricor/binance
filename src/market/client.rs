use binance::{api::Binance, futures::market::FuturesMarket};
use rust_decimal::prelude::FromPrimitive;

use super::error::MarketClientError;
use crate::noun::*;

type MarketClientResult<T> = Result<T, MarketClientError>;

pub struct MarketClient {
    client: FuturesMarket,
}

impl MarketClient {
    pub fn new() -> Self {
        let client = FuturesMarket::new(None, None);
        Self { client }
    }

    pub async fn price(&self, symbol: &Symbol) -> MarketClientResult<Price> {
        match self.client.get_price(symbol).await {
            Ok(v) => {
                if let Some(price) = Decimal::from_f64(v.price) {
                    return Ok(price);
                } else {
                    Err(MarketClientError::Decimal(v.price.to_string()))
                }
            }
            Err(e) => Err(MarketClientError::Client(e.to_string())),
        }
    }
}

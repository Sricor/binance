use binance::{api::Binance, futures::account::FuturesAccount};
use rust_decimal::prelude::ToPrimitive;

use super::{error::SpotClientError, Spot};
use crate::noun::*;

type SpotClientResult<T> = Result<T, SpotClientError>;

// ===== Spot Client =====
pub struct SpotClient {
    option: Option<SpotClientOption>,

    client: FuturesAccount,
}

impl SpotClient {
    pub fn new(api_key: String, secret_key: String, option: Option<SpotClientOption>) -> Self {
        let client = FuturesAccount::new(Some(api_key.clone()), Some(secret_key.clone()));
        Self { option, client }
    }
}

pub struct SpotClientOption {
    is_production: bool,
}

#[derive(Debug, PartialEq)]
pub struct SpotBuying {
    pub amount_spent: Amount,
    pub buying_quantity: Quantity,
    pub holding_quantity: Quantity,
    pub quantity_after_transaction: Quantity,
}

#[derive(Debug, PartialEq)]
pub struct SpotSelling {
    pub amount_income: Amount,
    pub amount_income_after_commission: Amount,
    pub selling_quantity: Quantity,
    pub quantity_after_transaction: Quantity,
}

impl SpotClient {
    pub fn is_production(&self) -> bool {
        match &self.option {
            Some(v) => v.is_production,
            None => false,
        }
    }

    pub async fn buy(
        &self,
        spot: &Spot,
        price: &Price,
        quantity: &Quantity,
    ) -> SpotClientResult<SpotBuying> {
        let quantity_with_precision = spot.transaction_quantity_with_precision(quantity);
        if !spot.is_allow_transaction(price, &quantity_with_precision) {
            return Err(SpotClientError::Trading(String::from(
                "maximum transaction amount not reached",
            )));
        }
        let amount_spent = spot.buying_spent_amount(price, &quantity_with_precision);
        let holding_quantity = spot.buying_quantity_with_commission(&quantity_with_precision);

        let result = SpotBuying {
            amount_spent,
            holding_quantity,
            buying_quantity: quantity_with_precision,
            quantity_after_transaction: quantity - quantity_with_precision,
        };

        if self.is_production() {
            let buy = self
                .client
                .market_buy(spot.symbol(), quantity.to_f64().unwrap())
                .await;
            if let Err(e) = buy {
                return Err(SpotClientError::Trading(e.to_string()));
            }
        }

        Ok(result)
    }

    pub async fn sell(
        &self,
        spot: &Spot,
        price: &Price,
        quantity: &Quantity,
    ) -> SpotClientResult<SpotSelling> {
        let quantity_with_precision = spot.transaction_quantity_with_precision(quantity);
        if !spot.is_allow_transaction(price, quantity) {
            return Err(SpotClientError::Trading(String::from(
                "maximum transaction amount not reached",
            )));
        }
        let amount = spot.selling_income_amount(price, &quantity_with_precision);

        let result = SpotSelling {
            amount_income: amount,
            amount_income_after_commission: spot.selling_amount_with_commission(&amount),
            selling_quantity: quantity_with_precision,
            quantity_after_transaction: quantity - quantity_with_precision,
        };

        if self.is_production() {
            let sell = self
                .client
                .market_sell(spot.symbol(), quantity.to_f64().unwrap())
                .await;
            if let Err(e) = sell {
                return Err(SpotClientError::Trading(e.to_string()));
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::prelude::FromPrimitive;

    use super::*;

    fn new_client() -> SpotClient {
        SpotClient::new("".into(), "".into(), None)
    }

    fn btc_spot() -> Spot {
        Spot {
            symbol: "BTCUSDT".into(),
            transaction_quantity_precision: 5,
            holding_quantity_precision: 7, // BTC Precision
            amount_income_precision: 8,    // USDT Precision
            minimum_transaction_amount: Decimal::from(5),
            buying_commission: Decimal::from_f64(0.001).unwrap(),
            selling_commission: Decimal::from_f64(0.001).unwrap(),
        }
    }

    fn eth_spot() -> Spot {
        Spot {
            symbol: "ETHUSDT".into(),
            transaction_quantity_precision: 4,
            holding_quantity_precision: 7, // ETH Precision
            amount_income_precision: 8,    // USDT Precision
            minimum_transaction_amount: Decimal::from(5),
            buying_commission: Decimal::from_f64(0.001).unwrap(),
            selling_commission: Decimal::from_f64(0.001).unwrap(),
        }
    }

    #[tokio::test]
    async fn test_dev_buy() {
        let client = new_client();
        let buying = client
            .buy(
                &btc_spot(),
                &Decimal::from_f64(43145.42).unwrap(),
                &Decimal::from_f64(0.0015).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotBuying {
            amount_spent: Decimal::from_f64(64.71813).unwrap(),
            buying_quantity: Decimal::from_f64(0.0015).unwrap(),
            holding_quantity: Decimal::from_f64(0.0014985).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.0).unwrap(),
        };
        assert_eq!(buying, assert);

        let client = new_client();
        let buying = client
            .buy(
                &btc_spot(),
                &Decimal::from_f64(43145.42).unwrap(),
                &Decimal::from_f64(0.00159858).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotBuying {
            amount_spent: Decimal::from_f64(68.6012178).unwrap(),
            buying_quantity: Decimal::from_f64(0.00159).unwrap(),
            holding_quantity: Decimal::from_f64(0.0015884).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.00000858).unwrap(),
        };
        assert_eq!(buying, assert);

        let client = new_client();
        let buying = client
            .buy(
                &eth_spot(),
                &Decimal::from_f64(2596.04).unwrap(),
                &Decimal::from_f64(0.079).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotBuying {
            amount_spent: Decimal::from_f64(205.087160).unwrap(),
            buying_quantity: Decimal::from_f64(0.0790).unwrap(),
            holding_quantity: Decimal::from_f64(0.0789210).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.0).unwrap(),
        };
        assert_eq!(buying, assert);

        let client = new_client();
        let buying = client
            .buy(
                &eth_spot(),
                &Decimal::from_f64(2596.04).unwrap(),
                &Decimal::from_f64(0.0791531).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotBuying {
            amount_spent: Decimal::from_f64(205.346764).unwrap(),
            buying_quantity: Decimal::from_f64(0.0791).unwrap(),
            holding_quantity: Decimal::from_f64(0.0790209).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.0000531).unwrap(),
        };
        assert_eq!(buying, assert);
    }

    #[tokio::test]
    async fn test_dev_sell() {
        let client = new_client();
        let buying = client
            .sell(
                &btc_spot(),
                &Decimal::from_f64(42991.10).unwrap(),
                &Decimal::from_f64(0.00349).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotSelling {
            amount_income: Decimal::from_f64(150.038939).unwrap(),
            amount_income_after_commission: Decimal::from_f64(149.88890006).unwrap(),
            selling_quantity: Decimal::from_f64(0.00349).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.0).unwrap(),
        };
        assert_eq!(buying, assert);

        let client = new_client();
        let buying = client
            .sell(
                &btc_spot(),
                &Decimal::from_f64(42991.10).unwrap(),
                &Decimal::from_f64(0.00349135).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotSelling {
            amount_income: Decimal::from_f64(150.038939).unwrap(),
            amount_income_after_commission: Decimal::from_f64(149.88890006).unwrap(),
            selling_quantity: Decimal::from_f64(0.00349).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.00000135).unwrap(),
        };
        assert_eq!(buying, assert);

        let client = new_client();
        let buying = client
            .sell(
                &eth_spot(),
                &Decimal::from_f64(2652.01).unwrap(),
                &Decimal::from_f64(0.1056).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotSelling {
            amount_income: Decimal::from_f64(280.052256).unwrap(),
            amount_income_after_commission: Decimal::from_f64(279.77220374).unwrap(),
            selling_quantity: Decimal::from_f64(0.1056).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.0).unwrap(),
        };
        assert_eq!(buying, assert);

        let client = new_client();
        let buying = client
            .sell(
                &eth_spot(),
                &Decimal::from_f64(2652.01).unwrap(),
                &Decimal::from_f64(0.105136).unwrap(),
            )
            .await
            .unwrap();
        let assert = SpotSelling {
            amount_income: Decimal::from_f64(278.726251).unwrap(),
            amount_income_after_commission: Decimal::from_f64(278.44752475).unwrap(),
            selling_quantity: Decimal::from_f64(0.1051).unwrap(),
            quantity_after_transaction: Decimal::from_f64(0.000036).unwrap(),
        };
        assert_eq!(buying, assert);
    }
}

use binance::{api::Binance, futures::account::FuturesAccount};
use rust_decimal::prelude::ToPrimitive;

use super::{error::SpotClientError, Spot};
use crate::{
    noun::*,
    strategy::{Master, Position, PositionSide, Strategy, Treasurer},
};

type SpotClientResult<T> = Result<T, SpotClientError>;

// ===== Spot Client =====
pub struct SpotClient {
    spot: Spot,
    option: Option<SpotClientOption>,

    client: FuturesAccount,
}

impl SpotClient {
    pub fn new(
        api_key: String,
        secret_key: String,
        spot: Spot,
        option: Option<SpotClientOption>,
    ) -> Self {
        let client = FuturesAccount::new(Some(api_key.clone()), Some(secret_key.clone()));
        Self {
            spot,
            option,
            client,
        }
    }
}

pub struct SpotClientOption {
    // Note that when true all transactions will be submitted to the exchange
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

    pub async fn buy(&self, price: &Price, quantity: &Quantity) -> SpotClientResult<SpotBuying> {
        let quantity_with_precision = self.spot.transaction_quantity_with_precision(quantity);
        if !self
            .spot
            .is_allow_transaction(price, &quantity_with_precision)
        {
            return Err(SpotClientError::Trading(String::from(
                "maximum transaction amount not reached",
            )));
        }
        let amount_spent = self
            .spot
            .buying_spent_amount(price, &quantity_with_precision);
        let holding_quantity = self
            .spot
            .buying_quantity_with_commission(&quantity_with_precision);

        let result = SpotBuying {
            amount_spent,
            holding_quantity,
            buying_quantity: quantity_with_precision,
            quantity_after_transaction: quantity - quantity_with_precision,
        };

        if self.is_production() {
            let buy = self
                .client
                .market_buy(self.spot.symbol(), quantity.to_f64().unwrap())
                .await;
            if let Err(e) = buy {
                return Err(SpotClientError::Trading(e.to_string()));
            }
        }

        Ok(result)
    }

    pub async fn sell(&self, price: &Price, quantity: &Quantity) -> SpotClientResult<SpotSelling> {
        let quantity_with_precision = self.spot.transaction_quantity_with_precision(quantity);
        if !self.spot.is_allow_transaction(price, quantity) {
            return Err(SpotClientError::Trading(String::from(
                "maximum transaction amount not reached",
            )));
        }
        let amount = self
            .spot
            .selling_income_amount(price, &quantity_with_precision);

        let result = SpotSelling {
            amount_income: amount,
            amount_income_after_commission: self.spot.selling_amount_with_commission(&amount),
            selling_quantity: quantity_with_precision,
            quantity_after_transaction: quantity - quantity_with_precision,
        };

        if self.is_production() {
            let sell = self
                .client
                .market_sell(self.spot.symbol(), quantity.to_f64().unwrap())
                .await;
            if let Err(e) = sell {
                return Err(SpotClientError::Trading(e.to_string()));
            }
        }

        Ok(result)
    }
}

#[allow(refining_impl_trait)]
impl Master for SpotClient {
    async fn trap(
        &self,
        price: &Price,
        strategy: &(impl Strategy + Send + Sync),
        treasurer: &(impl Treasurer + Send + Sync),
    ) -> SpotClientResult<()> {
        if strategy.is_completed() {
            return Err(SpotClientError::Strategy(String::from(
                "strategy completed",
            )));
        }

        if let Some(sell_list) = strategy.predictive_sell(price).await {
            for p in sell_list.iter() {
                let selling = self.sell(price, p.quantity()).await?;

                // TODO: return leave quantity
                strategy
                    .update_position(&PositionSide::Decrease(p.clone()))
                    .await;

                treasurer
                    .transfer_in(&selling.amount_income_after_commission)
                    .await;
            }
        }

        if let Some(buy_amount) = strategy.predictive_buy(price).await {
            let buy_quantity = self.spot.buying_quantity_by_amount(price, &buy_amount);
            let buying = self.buy(price, &buy_quantity).await?;

            let position =
                Position::new(price.clone(), buying.amount_spent, buying.holding_quantity);
            strategy
                .update_position(&PositionSide::Increase(position))
                .await;

            treasurer.transfer_out(&buying.amount_spent).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::prelude::FromPrimitive;

    use crate::{strategy::strategy::Percentage, treasurer::Prosperity};

    use super::*;

    fn new_client(spot: Spot) -> SpotClient {
        SpotClient::new("".into(), "".into(), spot, None)
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
        let client = new_client(btc_spot());
        let buying = client
            .buy(
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

        let client = new_client(btc_spot());
        let buying = client
            .buy(
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

        let client = new_client(eth_spot());
        let buying = client
            .buy(
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

        let client = new_client(eth_spot());
        let buying = client
            .buy(
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
        let client = new_client(btc_spot());
        let buying = client
            .sell(
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

        let client = new_client(btc_spot());
        let buying = client
            .sell(
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

        let client = new_client(eth_spot());
        let buying = client
            .sell(
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

        let client = new_client(eth_spot());
        let buying = client
            .sell(
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

    fn predict_price_one() -> Vec<Price> {
        vec![
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(101.0).unwrap(),
            Decimal::from_f64(101.5).unwrap(),
            Decimal::from_f64(102.3).unwrap(),
            Decimal::from_f64(100.9).unwrap(),
            Decimal::from_f64(99.58).unwrap(),
        ]
    }

    fn predict_price_two() -> Vec<Price> {
        vec![
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(99.23).unwrap(),
            Decimal::from_f64(98.52).unwrap(),
            Decimal::from_f64(97.45).unwrap(),
            Decimal::from_f64(96.67).unwrap(),
            Decimal::from_f64(93.23).unwrap(),
            Decimal::from_f64(92.95).unwrap(),
            Decimal::from_f64(90.94).unwrap(),
        ]
    }

    fn predict_price_three() -> Vec<Price> {
        vec![
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(101.0).unwrap(),
            Decimal::from_f64(103.5).unwrap(),
            Decimal::from_f64(106.9).unwrap(),
            Decimal::from_f64(108.9).unwrap(),
            Decimal::from_f64(111.9).unwrap(),
            Decimal::from_f64(109.5).unwrap(),
            Decimal::from_f64(103.2).unwrap(),
            Decimal::from_f64(102.5).unwrap(),
            Decimal::from_f64(100.3).unwrap(),
            Decimal::from_f64(100.0).unwrap(),
        ]
    }

    #[tokio::test]
    async fn test_strategy_trap_percentage_one() {
        let price = predict_price_one();
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let strategy = Percentage::new(
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(0.01).unwrap(),
            None,
            None,
        );

        for p in price.iter() {
            let result = client.trap(p, &strategy, &treasurer).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), true);
        assert_eq!(strategy.positions().await.is_empty(), true);
        assert_eq!(
            treasurer.balance().await,
            Decimal::from_f64(1.29710150).unwrap()
        );
    }

    #[tokio::test]
    async fn test_strategy_trap_percentage_two() {
        let price = predict_price_two();
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let strategy = Percentage::new(
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(0.01).unwrap(),
            None,
            None,
        );

        for p in price.iter() {
            let result = client.trap(p, &strategy, &treasurer).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), false);
        assert_eq!(strategy.positions().await.is_empty(), false);
        assert_eq!(
            treasurer.balance().await,
            Decimal::from_f64(-100.00000).unwrap()
        );
    }

    #[tokio::test]
    async fn test_strategy_trap_percentage_three() {
        let price = predict_price_three();
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let strategy = Percentage::new(
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(0.02).unwrap(),
            None,
            None,
        );

        for p in price.iter() {
            let result = client.trap(p, &strategy, &treasurer).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), true);
        assert_eq!(strategy.positions().await.is_empty(), true);
        assert_eq!(
            treasurer.balance().await,
            Decimal::from_f64(3.29310350).unwrap()
        );
    }

    #[tokio::test]
    async fn test_strategy_trap_percentage_stop_loss() {
        let price = predict_price_two();
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let strategy = Percentage::new(
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(0.01).unwrap(),
            Some(Decimal::from_f64(0.03).unwrap()),
            None,
        );

        for p in price.iter() {
            let result = client.trap(p, &strategy, &treasurer).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), true);
        assert_eq!(strategy.positions().await.is_empty(), true);
        assert_eq!(
            treasurer.balance().await,
            Decimal::from_f64(-0.96836077).unwrap()
        );
    }

    #[tokio::test]
    async fn test_strategy_trap_percentage_start_buying() {
        let price = predict_price_two();
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let strategy = Percentage::new(
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(0.01).unwrap(),
            Some(Decimal::from_f64(0.03).unwrap()),
            Some(Decimal::from_f64(99.0).unwrap()),
        );

        for p in price.iter() {
            let result = client.trap(p, &strategy, &treasurer).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), true);
        assert_eq!(strategy.positions().await.is_empty(), true);
        assert_eq!(
            treasurer.balance().await,
            Decimal::from_f64(-0.96836077).unwrap()
        );
    }

    #[tokio::test]
    async fn test_strategy_trap_percentage_start_buying_two() {
        let price = predict_price_two();
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let strategy = Percentage::new(
            Decimal::from_f64(100.0).unwrap(),
            Decimal::from_f64(0.01).unwrap(),
            Some(Decimal::from_f64(0.03).unwrap()),
            Some(Decimal::from_f64(101.0).unwrap()),
        );

        for p in price.iter() {
            let result = client.trap(p, &strategy, &treasurer).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), false);
        assert_eq!(strategy.positions().await.is_empty(), true);
        assert_eq!(treasurer.balance().await, Decimal::from_f64(0.0).unwrap());
    }
}

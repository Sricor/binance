use binance::{
    account::{Account, OrderRequest},
    api::Binance,
};
use rust_decimal::prelude::ToPrimitive;

use super::{error::SpotClientError, Spot, SpotBuying, SpotSelling};
use crate::{
    noun::*,
    strategy::{Master, Order, PositionSide, Strategy, Treasurer},
};

type SpotClientResult<T> = Result<T, SpotClientError>;

// ===== Spot Client =====
pub struct SpotClient {
    spot: Spot,
    option: Option<SpotClientOption>,

    client: Account,
}

impl SpotClient {
    pub fn new(
        api_key: String,
        secret_key: String,
        spot: Spot,
        option: Option<SpotClientOption>,
    ) -> Self {
        let client = Account::new(Some(api_key.clone()), Some(secret_key.clone()));
        Self {
            spot,
            option,
            client,
        }
    }
}

pub struct SpotClientOption {
    // Note that when true all transactions will be submitted to the exchange
    pub is_production: bool,
}

impl SpotClient {
    pub fn is_production(&self) -> bool {
        match &self.option {
            Some(v) => v.is_production,
            None => false,
        }
    }

    pub async fn buy(&self, price: &Price, quantity: &Quantity) -> SpotClientResult<SpotBuying> {
        let buying_quantity = self.spot.transaction_quantity_with_precision(quantity);
        self.is_allow_transaction(price, &buying_quantity)?;

        if self.is_production() {
            let buy = self
                .client
                .place_order(OrderRequest {
                    symbol: self.spot.symbol().clone(),
                    side: binance::rest_model::OrderSide::Buy,
                    order_type: binance::rest_model::OrderType::Market,
                    quantity: Some(buying_quantity.to_f64().unwrap()),
                    price: None,
                    ..OrderRequest::default()
                })
                .await;

            if let Err(e) = buy {
                return Err(SpotClientError::Trading(e.to_string()));
            }
        }

        Ok(self.calculator_buying(price, &buying_quantity))
    }

    pub async fn sell(&self, price: &Price, quantity: &Quantity) -> SpotClientResult<SpotSelling> {
        let selling_quantity = self.spot.transaction_quantity_with_precision(quantity);
        self.is_allow_transaction(price, &selling_quantity)?;

        if self.is_production() {
            let sell = self
                .client
                .place_order(OrderRequest {
                    symbol: self.spot.symbol().clone(),
                    side: binance::rest_model::OrderSide::Sell,
                    order_type: binance::rest_model::OrderType::Market,
                    quantity: Some(selling_quantity.to_f64().unwrap()),
                    price: None,
                    ..OrderRequest::default()
                })
                .await;

            if let Err(e) = sell {
                return Err(SpotClientError::Trading(e.to_string()));
            }
        }

        Ok(self.calculator_selling(price, &selling_quantity))
    }

    pub async fn test_buy(&self, _price: &Price, quantity: &Quantity) -> SpotClientResult<()> {
        let buy = self
            .client
            .place_test_order(OrderRequest {
                symbol: self.spot.symbol().clone(),
                side: binance::rest_model::OrderSide::Buy,
                order_type: binance::rest_model::OrderType::Market,
                quantity: Some(quantity.to_f64().unwrap()),
                price: None,
                ..OrderRequest::default()
            })
            .await;

        if let Err(e) = buy {
            return Err(SpotClientError::Trading(e.to_string()));
        }
        Ok(())
    }

    pub async fn test_sell(&self, _price: &Price, quantity: &Quantity) -> SpotClientResult<()> {
        let buy = self
            .client
            .place_test_order(OrderRequest {
                symbol: self.spot.symbol().clone(),
                side: binance::rest_model::OrderSide::Sell,
                order_type: binance::rest_model::OrderType::Market,
                quantity: Some(quantity.to_f64().unwrap()),
                price: None,
                ..OrderRequest::default()
            })
            .await;

        if let Err(e) = buy {
            return Err(SpotClientError::Trading(e.to_string()));
        }
        Ok(())
    }

    fn calculator_buying(&self, price: &Price, buying_quantity: &Quantity) -> SpotBuying {
        let spent = self.spot.buying_spent_amount(price, buying_quantity);
        let quantity_after_commission = self.spot.buying_quantity_with_commission(buying_quantity);

        SpotBuying {
            spent,
            price: price.clone(),
            quantity: buying_quantity.clone(),
            quantity_after_commission,
        }
    }

    fn calculator_selling(&self, price: &Price, selling_quantity: &Quantity) -> SpotSelling {
        let selling_income = self.spot.selling_income_amount(price, selling_quantity);
        let income_after_commission = self.spot.selling_amount_with_commission(&selling_income);

        SpotSelling {
            price: price.clone(),
            quantity: selling_quantity.clone(),
            income: selling_income,
            income_after_commission,
        }
    }

    fn is_allow_transaction(&self, price: &Price, quantity: &Quantity) -> SpotClientResult<()> {
        if !self
            .spot
            .is_reached_minimum_transaction_limit(price, quantity)
        {
            return Err(SpotClientError::Trading(String::from(
                "minimum transaction amount not reached",
            )));
        }

        Ok(())
    }
}

#[allow(refining_impl_trait)]
impl Master for SpotClient {
    type Item = ();

    async fn trap<S, T>(
        &self,
        price: &Price,
        strategy: &S,
        treasurer: Option<&T>,
    ) -> SpotClientResult<Self::Item>
    where
        S: Strategy + Send + Sync,
        T: Treasurer + Send + Sync,
    {
        if strategy.is_completed() {
            return Err(SpotClientError::Strategy(String::from(
                "strategy completed",
            )));
        }

        if let Some(sell_list) = strategy.predictive_sell(price).await {
            if !sell_list.is_empty() {
                for o in sell_list.iter() {
                    let selling = self.sell(price, o.quantity()).await?;

                    // TODO: return leave quantity
                    strategy
                        .update_position(&PositionSide::Decrease(o.clone()))
                        .await;

                    if let Some(t) = treasurer {
                        t.transfer_in(&selling.income_after_commission).await;
                    }
                }

                return Ok(());
            }
        }

        if let Some(buy_amount) = strategy.predictive_buy(price).await {
            let buy_quantity = self.spot.buying_quantity_by_amount(price, &buy_amount);
            let buying = self.buy(price, &buy_quantity).await?;

            let order = Order::new(
                price.clone(),
                buying.spent,
                buying.quantity_after_commission,
            );
            strategy
                .update_position(&PositionSide::Increase(order))
                .await;

            if let Some(t) = treasurer {
                t.transfer_out(&buying.spent).await;
            }

            return Ok(());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::prelude::FromPrimitive;

    use crate::{
        strategy::strategy::{Bound, BoundPosition, Grid, Percentage},
        treasurer::Prosperity,
    };

    use super::*;

    fn to_decimal(value: f64) -> Decimal {
        Decimal::from_f64(value).unwrap()
    }

    fn new_client(spot: Spot) -> SpotClient {
        SpotClient::new("".into(), "".into(), spot, None)
    }

    fn btc_spot() -> Spot {
        Spot {
            symbol: "BTCUSDT".into(),
            transaction_quantity_precision: 5,
            quantity_precision: 7, // BTC Precision
            amount_precision: 8,   // USDT Precision
            minimum_transaction_amount: Decimal::from(5),
            buying_commission: Decimal::from_f64(0.001).unwrap(),
            selling_commission: Decimal::from_f64(0.001).unwrap(),
        }
    }

    fn eth_spot() -> Spot {
        Spot {
            symbol: "ETHUSDT".into(),
            transaction_quantity_precision: 4,
            quantity_precision: 7, // ETH Precision
            amount_precision: 8,   // USDT Precision
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
            price: Decimal::from_f64(43145.42).unwrap(),
            spent: Decimal::from_f64(64.71813).unwrap(),
            quantity: Decimal::from_f64(0.0015).unwrap(),
            quantity_after_commission: Decimal::from_f64(0.0014985).unwrap(),
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
            price: Decimal::from_f64(43145.42).unwrap(),
            spent: Decimal::from_f64(68.6012178).unwrap(),
            quantity: Decimal::from_f64(0.00159).unwrap(),
            quantity_after_commission: Decimal::from_f64(0.0015884).unwrap(),
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
            price: Decimal::from_f64(2596.04).unwrap(),
            spent: Decimal::from_f64(205.087160).unwrap(),
            quantity: Decimal::from_f64(0.0790).unwrap(),
            quantity_after_commission: Decimal::from_f64(0.0789210).unwrap(),
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
            price: Decimal::from_f64(2596.04).unwrap(),
            spent: Decimal::from_f64(205.346764).unwrap(),
            quantity: Decimal::from_f64(0.0791).unwrap(),
            quantity_after_commission: Decimal::from_f64(0.0790209).unwrap(),
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
            price: Decimal::from_f64(42991.10).unwrap(),
            income: Decimal::from_f64(150.038939).unwrap(),
            income_after_commission: Decimal::from_f64(149.88890006).unwrap(),
            quantity: Decimal::from_f64(0.00349).unwrap(),
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
            price: Decimal::from_f64(42991.10).unwrap(),
            income: Decimal::from_f64(150.038939).unwrap(),
            income_after_commission: Decimal::from_f64(149.88890006).unwrap(),
            quantity: Decimal::from_f64(0.00349).unwrap(),
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
            price: Decimal::from_f64(2652.01).unwrap(),
            income: Decimal::from_f64(280.052256).unwrap(),
            income_after_commission: Decimal::from_f64(279.77220374).unwrap(),
            quantity: Decimal::from_f64(0.1056).unwrap(),
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
            price: Decimal::from_f64(2652.01).unwrap(),
            income: Decimal::from_f64(278.726251).unwrap(),
            income_after_commission: Decimal::from_f64(278.44752475).unwrap(),
            quantity: Decimal::from_f64(0.1051).unwrap(),
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

    fn predict_price_four() -> Vec<Price> {
        vec![
            to_decimal(54.90),
            to_decimal(64.90),
            to_decimal(65.10),
            to_decimal(74.90),
            to_decimal(75.10),
            to_decimal(85.10),
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
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
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
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
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
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
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
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
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
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
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
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), false);
        assert_eq!(strategy.positions().await.is_empty(), true);
        assert_eq!(treasurer.balance().await, Decimal::from_f64(0.0).unwrap());
    }

    #[tokio::test]
    async fn test_strategy_trap_grid() {
        let price = predict_price_four();
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let positions = BoundPosition::with_copies(Bound(to_decimal(50.0), to_decimal(90.0)), 4);
        let strategy = Grid::new(to_decimal(100.0), positions);

        for p in price.iter() {
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), false);
        assert_eq!(treasurer.balance().await, to_decimal(11.80321024));
    }

    #[tokio::test]
    async fn test_strategy_trap_grid_predictive_lowest_profit_price() {
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let positions = BoundPosition::with_copies(Bound(to_decimal(50.0), to_decimal(90.0)), 4);
        let strategy = Grid::new(to_decimal(100.0), positions);

        for p in strategy.predictive_lowest_profit_price().iter() {
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), false);
        assert_eq!(treasurer.balance().await, to_decimal(11.567461825));
    }

    #[tokio::test]
    async fn test_strategy_trap_grid_predictive_highest_profit_price() {
        let client = new_client(btc_spot());
        let treasurer = Prosperity::new(None);
        let positions = BoundPosition::with_copies(Bound(to_decimal(50.0), to_decimal(90.0)), 4);
        let strategy = Grid::new(to_decimal(100.0), positions);

        for p in strategy.predictive_highest_profit_price().iter() {
            let result = client.trap(p, &strategy, Some(&treasurer)).await;
            if let Err(e) = result {
                println!("{e}");
            }
        }

        assert_eq!(strategy.is_completed(), false);
        assert_eq!(treasurer.balance().await, to_decimal(25.25451036));
    }
}

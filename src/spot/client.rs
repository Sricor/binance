use std::sync::Arc;

use binance::{
    account::{Account, OrderRequest},
    api::Binance,
    market::Market,
};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};

use super::{error::SpotClientError, Spot, SpotBuying, SpotSelling};
use crate::{
    noun::*,
    strategy::{AmountPoint, ClosureFuture, Exchanger, PricePoint, QuantityPoint},
};

type SpotClientResult<T> = Result<T, SpotClientError>;

// ===== Spot Client =====
pub struct SpotClient {
    spot: Spot,
    option: Option<SpotClientOption>,

    pub market: Market,
    pub client: Account,
}

impl SpotClient {
    pub fn new(
        api_key: String,
        secret_key: String,
        spot: Spot,
        option: Option<SpotClientOption>,
    ) -> Self {
        let client = Account::new(Some(api_key.clone()), Some(secret_key.clone()));
        let market = Market::new(None, None);
        Self {
            spot,
            option,
            client,
            market,
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

    pub async fn price(&self) -> SpotClientResult<Price> {
        match self.market.get_price(self.spot.symbol()).await {
            Ok(v) => {
                let price = Decimal::from_f64(v.price)
                    .ok_or(SpotClientError::Decimal(v.price.to_string()))?;

                Ok(price)
            }
            Err(e) => Err(SpotClientError::Price(e.to_string())),
        }
    }

    pub async fn buy(&self, price: &Price, amount: &Amount) -> SpotClientResult<SpotBuying> {
        let buying_quantity = self.spot.buying_quantity_by_amount(price, amount);
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
                "Minimum transaction amount not reached",
            )));
        }

        Ok(())
    }
}

impl Exchanger for SpotClient {
    fn spawn_buy(self: &Arc<Self>) -> impl Fn(Price, Amount) -> ClosureFuture<QuantityPoint> {
        let result = move |price: Price, amount: Amount| -> ClosureFuture<QuantityPoint> {
            let client: Arc<SpotClient> = self.clone();

            let f = async move {
                let quantity = client.buy(&price, &amount).await?.quantity_after_commission;

                Ok(QuantityPoint::new(quantity))
            };

            Box::pin(f)
        };

        result
    }

    fn spawn_sell(self: &Arc<Self>) -> impl Fn(Price, Quantity) -> ClosureFuture<AmountPoint> {
        let result = move |price: Price, quantity: Quantity| -> ClosureFuture<AmountPoint> {
            let client = self.clone();

            let f = async move {
                let income = client
                    .sell(&price, &quantity)
                    .await?
                    .income_after_commission;

                Ok(AmountPoint::new(income))
            };

            Box::pin(f)
        };

        result
    }

    fn spawn_price(self: &Arc<Self>) -> impl Fn() -> ClosureFuture<PricePoint> {
        let result = move || -> ClosureFuture<PricePoint> {
            let client = self.clone();

            let f = async move {
                let price = client.price().await?;

                Ok(PricePoint::new(price))
            };

            Box::pin(f)
        };

        result
    }
}

#[cfg(test)]
mod tests_count_leak {
    use super::super::tests_general::*;
    use super::*;

    fn simple_client(spot: Spot) -> SpotClient {
        SpotClient::new(String::from("null"), String::from("null"), spot, None)
    }

    #[tokio::test]
    async fn test_spwan_count() {
        let client = Arc::new(simple_client(btc_spot()));
        let buy = client.spawn_buy();
        assert_eq!(Arc::strong_count(&client), 1);

        let f = buy(decimal(1.0), decimal(20.0));
        assert_eq!(Arc::strong_count(&client), 2);

        f.await.unwrap();
        assert_eq!(Arc::strong_count(&client), 1);

        let sell = client.spawn_sell();
        assert_eq!(Arc::strong_count(&client), 1);

        let f = sell(decimal(1.0), decimal(20.0));
        assert_eq!(Arc::strong_count(&client), 2);

        f.await.unwrap();
        assert_eq!(Arc::strong_count(&client), 1);

        let f = sell(decimal(1.0), decimal(20.0));
        assert_eq!(Arc::strong_count(&client), 2);

        drop(f);
        assert_eq!(Arc::strong_count(&client), 1);
    }

    #[tokio::test]
    async fn test_spwan_multi_count() {
        let client = Arc::new(simple_client(btc_spot()));
        let number = 10;

        let mut vec = Vec::new();

        for _ in 0..number {
            vec.push(client.spawn_buy()(decimal(1.0), decimal(20.0)))
        }
        assert_eq!(Arc::strong_count(&client), number + 1);

        for i in vec.into_iter() {
            i.await.unwrap();
        }
        assert_eq!(Arc::strong_count(&client), 1);

        let mut vec = Vec::new();

        for _ in 0..number {
            vec.push(client.spawn_sell()(decimal(1.0), decimal(20.0)))
        }
        assert_eq!(Arc::strong_count(&client), number + 1);

        for i in vec.into_iter() {
            i.await.unwrap();
        }
        assert_eq!(Arc::strong_count(&client), 1);
    }
}

#[cfg(test)]
mod tests_client {
    use tracing_test::traced_test;

    use super::super::tests_general::*;
    use super::*;

    fn simple_client(spot: Spot) -> SpotClient {
        SpotClient::new(String::from("null"), String::from("null"), spot, None)
    }

    #[tokio::test]
    async fn test_buying() {
        let client = simple_client(btc_spot());
        let buying = client
            .buy(&decimal(43145.42), &decimal(500.0))
            .await
            .unwrap();
        let assert = SpotBuying {
            price: decimal(43145.42),
            spent: decimal(499.6239636),
            quantity: decimal(0.01158),
            quantity_after_commission: decimal(0.0115684),
        };
        assert_eq!(buying, assert);

        let client = simple_client(btc_spot());
        let buying = client
            .buy(&decimal(43145.42), &decimal(1000.0))
            .await
            .unwrap();
        let assert = SpotBuying {
            price: decimal(43145.42),
            spent: decimal(999.6793814),
            quantity: decimal(0.02317),
            quantity_after_commission: decimal(0.0231468),
        };
        assert_eq!(buying, assert);

        let client = simple_client(eth_spot());
        let buying = client
            .buy(&decimal(2596.04), &decimal(600.50))
            .await
            .unwrap();
        let assert = SpotBuying {
            price: decimal(2596.04),
            spent: decimal(600.464052),
            quantity: decimal(0.2313),
            quantity_after_commission: decimal(0.2310687),
        };
        assert_eq!(buying, assert);

        let client = simple_client(eth_spot());
        let buying = client
            .buy(&decimal(2596.04), &decimal(100.0))
            .await
            .unwrap();
        let assert = SpotBuying {
            price: decimal(2596.04),
            spent: decimal(99.947540),
            quantity: decimal(0.0385),
            quantity_after_commission: decimal(0.0384615),
        };
        assert_eq!(buying, assert);
    }

    // #[tokio::test]
    // async fn test_buying_with_quantity() {
    //     let client = simple_client(btc_spot());
    //     let buying = client
    //         .buy(&decimal(43145.42), &decimal(0.0015))
    //         .await
    //         .unwrap();
    //     let assert = SpotBuying {
    //         price: decimal(43145.42),
    //         spent: decimal(64.71813),
    //         quantity: decimal(0.0015),
    //         quantity_after_commission: decimal(0.0014985),
    //     };
    //     assert_eq!(buying, assert);

    //     let client = simple_client(btc_spot());
    //     let buying = client
    //         .buy(&decimal(43145.42), &decimal(0.00159858))
    //         .await
    //         .unwrap();
    //     let assert = SpotBuying {
    //         price: decimal(43145.42),
    //         spent: decimal(68.6012178),
    //         quantity: decimal(0.00159),
    //         quantity_after_commission: decimal(0.0015884),
    //     };
    //     assert_eq!(buying, assert);

    //     let client = simple_client(eth_spot());
    //     let buying = client
    //         .buy(&decimal(2596.04), &decimal(0.079))
    //         .await
    //         .unwrap();
    //     let assert = SpotBuying {
    //         price: decimal(2596.04),
    //         spent: decimal(205.087160),
    //         quantity: decimal(0.0790),
    //         quantity_after_commission: decimal(0.0789210),
    //     };
    //     assert_eq!(buying, assert);

    //     let client = simple_client(eth_spot());
    //     let buying = client
    //         .buy(&decimal(2596.04), &decimal(0.0791531))
    //         .await
    //         .unwrap();
    //     let assert = SpotBuying {
    //         price: decimal(2596.04),
    //         spent: decimal(205.346764),
    //         quantity: decimal(0.0791),
    //         quantity_after_commission: decimal(0.0790209),
    //     };
    //     assert_eq!(buying, assert);
    // }

    #[tokio::test]
    #[traced_test]
    async fn test_selling() {
        let client = simple_client(btc_spot());
        let buying = client
            .sell(&decimal(42991.10), &decimal(0.00349))
            .await
            .unwrap();
        let assert = SpotSelling {
            price: decimal(42991.10),
            income: decimal(150.038939),
            income_after_commission: decimal(149.88890006),
            quantity: decimal(0.00349),
        };
        assert_eq!(buying, assert);

        let client = simple_client(btc_spot());
        let buying = client
            .sell(&decimal(42991.10), &decimal(0.00349135))
            .await
            .unwrap();
        let assert = SpotSelling {
            price: decimal(42991.10),
            income: decimal(150.038939),
            income_after_commission: decimal(149.88890006),
            quantity: decimal(0.00349),
        };
        assert_eq!(buying, assert);

        let client = simple_client(eth_spot());
        let buying = client
            .sell(&decimal(2652.01), &decimal(0.1056))
            .await
            .unwrap();
        let assert = SpotSelling {
            price: decimal(2652.01),
            income: decimal(280.052256),
            income_after_commission: decimal(279.77220374),
            quantity: decimal(0.1056),
        };
        assert_eq!(buying, assert);

        let client = simple_client(eth_spot());
        let buying = client
            .sell(&decimal(2652.01), &decimal(0.105136))
            .await
            .unwrap();
        let assert = SpotSelling {
            price: decimal(2652.01),
            income: decimal(278.726251),
            income_after_commission: decimal(278.44752475),
            quantity: decimal(0.1051),
        };
        assert_eq!(buying, assert);
    }
}

//     use rust_decimal::prelude::FromPrimitive;
//     use tracing_test::traced_test;

//     use crate::{
//         strategy::strategy::{Grid, Percentage},
//         treasurer::Prosperity,
//     };

//     fn price(value: f64) -> PriceSignal {
//         PriceSignal::new(decimal(value))
//     }

//     fn to_decimal(value: f64) -> Decimal {
//         Decimal::from_f64(value).unwrap()
//     }

//     fn new_client(spot: Spot) -> SpotClient {
//         SpotClient::new("".into(), "".into(), spot, None)
//     }

//     fn predict_price_one() -> Vec<PriceSignal> {
//         vec![
//             price(100.0),
//             price(101.0),
//             price(101.5),
//             price(102.3),
//             price(100.9),
//             price(99.58),
//         ]
//     }

//     fn predict_price_two() -> Vec<PriceSignal> {
//         vec![
//             price(100.0),
//             price(99.23),
//             price(98.52),
//             price(97.45),
//             price(96.67),
//             price(93.23),
//             price(92.95),
//             price(90.94),
//         ]
//     }

//     fn predict_price_three() -> Vec<PriceSignal> {
//         vec![
//             price(100.0),
//             price(101.0),
//             price(103.5),
//             price(106.9),
//             price(108.9),
//             price(111.9),
//             price(109.5),
//             price(103.2),
//             price(102.5),
//             price(100.3),
//             price(100.0),
//         ]
//     }

//     fn predict_price_four() -> Vec<PriceSignal> {
//         vec![
//             price(54.90),
//             price(64.90),
//             price(65.10),
//             price(74.90),
//             price(75.10),
//             price(85.10),
//         ]
//     }

//     fn predict_price_five() -> Vec<Price> {
//         vec![
//             to_decimal(60.00),
//             to_decimal(60.00),
//             to_decimal(60.00),
//             to_decimal(60.00),
//             to_decimal(60.00),
//             to_decimal(60.00),
//         ]
//     }

//     fn predict_price_six() -> Vec<Price> {
//         vec![
//             to_decimal(95.00),
//             to_decimal(95.00),
//             to_decimal(95.00),
//             to_decimal(95.00),
//             to_decimal(95.00),
//             to_decimal(95.00),
//         ]
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_percentage_one() {
//         let price = predict_price_one();
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Percentage::new(
//             decimal(100.0).unwrap(),
//             decimal(0.01).unwrap(),
//             None,
//             None,
//         );

//         for p in price.iter() {
//             let result = client.trap(p, &strategy, Some(&treasurer)).await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), true);
//         assert_eq!(strategy.positions().await.is_empty(), true);
//         assert_eq!(
//             treasurer.balance().await,
//             decimal(1.29710150).unwrap()
//         );
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_percentage_two() {
//         let price = predict_price_two();
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Percentage::new(
//             decimal(100.0).unwrap(),
//             decimal(0.01).unwrap(),
//             None,
//             None,
//         );

//         for p in price.iter() {
//             let result = client.trap(p, &strategy, Some(&treasurer)).await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), false);
//         assert_eq!(strategy.positions().await.is_empty(), false);
//         assert_eq!(
//             treasurer.balance().await,
//             decimal(-100.00000).unwrap()
//         );
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_percentage_three() {
//         let price = predict_price_three();
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Percentage::new(
//             decimal(100.0).unwrap(),
//             decimal(0.02).unwrap(),
//             None,
//             None,
//         );

//         for p in price.iter() {
//             let result = client.trap(p, &strategy, Some(&treasurer)).await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), true);
//         assert_eq!(strategy.positions().await.is_empty(), true);
//         assert_eq!(
//             treasurer.balance().await,
//             decimal(3.29310350).unwrap()
//         );
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_percentage_stop_loss() {
//         let price = predict_price_two();
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Percentage::new(
//             decimal(100.0).unwrap(),
//             decimal(0.01).unwrap(),
//             Some(decimal(0.03).unwrap()),
//             None,
//         );

//         for p in price.iter() {
//             let result = client.trap(p, &strategy, Some(&treasurer)).await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), true);
//         assert_eq!(strategy.positions().await.is_empty(), true);
//         assert_eq!(
//             treasurer.balance().await,
//             decimal(-0.96836077).unwrap()
//         );
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_percentage_start_buying() {
//         let price = predict_price_two();
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Percentage::new(
//             decimal(100.0).unwrap(),
//             decimal(0.01).unwrap(),
//             Some(decimal(0.03).unwrap()),
//             Some(decimal(99.0).unwrap()),
//         );

//         for p in price.iter() {
//             let result = client.trap(p, &strategy, Some(&treasurer)).await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), true);
//         assert_eq!(strategy.positions().await.is_empty(), true);
//         assert_eq!(
//             treasurer.balance().await,
//             decimal(-0.96836077).unwrap()
//         );
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_percentage_start_buying_two() {
//         let price = predict_price_two();
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Percentage::new(
//             decimal(100.0).unwrap(),
//             decimal(0.01).unwrap(),
//             Some(decimal(0.03).unwrap()),
//             Some(decimal(101.0).unwrap()),
//         );

//         for p in price.iter() {
//             let result = client.trap(p, &strategy, Some(&treasurer)).await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), false);
//         assert_eq!(strategy.positions().await.is_empty(), true);
//         assert_eq!(treasurer.balance().await, decimal(0.0).unwrap());
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_grid() {
//         let price = predict_price_four();
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Grid::new(
//             to_decimal(100.0),
//             (to_decimal(50.0), to_decimal(90.0)),
//             4,
//             None,
//         );

//         for p in price.iter() {
//             let result = client.trap(p, &strategy, Some(&treasurer)).await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), false);
//         assert_eq!(treasurer.balance().await, to_decimal(11.80321024));
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_grid_predictive_lowest_profit_price() {
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Grid::new(
//             to_decimal(100.0),
//             (to_decimal(50.0), to_decimal(90.0)),
//             4,
//             None,
//         );

//         for p in strategy.predictive_lowest_profit_price().iter() {
//             let result = client
//                 .trap(&price(p.to_f64().unwrap()), &strategy, Some(&treasurer))
//                 .await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), false);
//         assert_eq!(treasurer.balance().await, to_decimal(11.567461825));
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_grid_predictive_highest_profit_price() {
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Grid::new(
//             to_decimal(100.0),
//             (to_decimal(50.0), to_decimal(90.0)),
//             4,
//             None,
//         );

//         for p in strategy.predictive_highest_profit_price().iter() {
//             let result = client
//                 .trap(&price(p.to_f64().unwrap()), &strategy, Some(&treasurer))
//                 .await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), false);
//         assert_eq!(treasurer.balance().await, to_decimal(25.25451036));
//     }

//     #[tokio::test]
//     #[traced_test]
//     async fn test_strategy_trap_grid_double_trading() {
//         let client = new_client(btc_spot());
//         let treasurer = Prosperity::new(None);
//         let strategy = Grid::new(
//             to_decimal(100.0),
//             (to_decimal(30.75), to_decimal(175.35)),
//             6,
//             None,
//         );

//         for p in predict_price_five().iter() {
//             let result = client
//                 .trap(&price(p.to_f64().unwrap()), &strategy, Some(&treasurer))
//                 .await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), false);
//         assert_eq!(treasurer.balance().await, to_decimal(-16.66620));

//         for p in predict_price_six().iter() {
//             let result = client
//                 .trap(&price(p.to_f64().unwrap()), &strategy, Some(&treasurer))
//                 .await;
//             if let Err(e) = result {
//                 println!("{e}");
//             }
//         }

//         assert_eq!(strategy.is_completed(), false);
//         assert_eq!(treasurer.balance().await, to_decimal(9.66898845));
//     }
// }

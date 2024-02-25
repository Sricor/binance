use std::pin::Pin;
use std::sync::Mutex;
use std::{error::Error, future::Future};

use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{Amount, AmountPoint, Price, PricePoint, Quantity, QuantityPoint};

pub type Position = Option<Quantity>;

#[derive(Debug, Serialize, Deserialize)]
pub struct LimitPosition {
    pub buying: Price,
    pub selling: Price,
    pub investment: Amount,
    pub position: Mutex<Position>,
}

impl LimitPosition {
    pub fn new(investment: Amount, buying: Price, selling: Price, position: Position) -> Self {
        Self {
            investment,
            buying,
            selling,
            position: Mutex::new(position),
        }
    }

    fn predictive_buying(&self, price: &Price) -> Option<Amount> {
        if &self.buying > price {
            let position = self.position.lock().unwrap();

            if let None = *position {
                return Some(self.investment);
            }
        }

        None
    }

    fn predictive_selling(&self, price: &Price) -> Option<Quantity> {
        if &self.selling < price {
            let position = self.position.lock().unwrap();

            if let Some(quantity) = *position {
                return Some(quantity.clone());
            }
        }

        None
    }

    fn update_position(&self, position: Position) {
        let mut source = self.position.lock().unwrap();

        *source = position;
    }
}

pub struct Limit {
    positions: Vec<LimitPosition>,
}

impl Limit {
    pub fn with_positions(positions: Vec<LimitPosition>) -> Self {
        Self { positions }
    }
}

impl StrategyFn for Limit {
    #[instrument(skip_all)]
    async fn trap<P, B, S>(&self, price: &P, buy: &B, sell: &S) -> Result<(), Box<dyn Error>>
    where
        P: Fn() -> ClosureFuture<PricePoint>,
        B: Fn(&Price, &Amount) -> ClosureFuture<QuantityPoint>,
        S: Fn(&Price, &Quantity) -> ClosureFuture<AmountPoint>,
    {
        let price = match price().await {
            Ok(v) => v.value().clone(),
            Err(e) => return Err(e),
        };

        for limit_position in self.positions.iter() {
            if let Some(quantity) = limit_position.predictive_selling(&price) {
                let _ = sell(&price, &quantity).await?;
                limit_position.update_position(None);

                continue;
            }

            if let Some(amount) = limit_position.predictive_buying(&price) {
                let quantity = buy(&price, &amount).await?;
                limit_position.update_position(Some(quantity.value().clone()));

                continue;
            }
        }

        Ok(())
    }
}

pub type ClosureFuture<T> = Pin<Box<dyn Future<Output = Result<T, Box<dyn Error>>> + Send>>;

pub trait StrategyFn {
    async fn trap<P, B, S>(&self, price: &P, buy: &B, sell: &S) -> Result<(), Box<dyn Error>>
    where
        P: Fn() -> ClosureFuture<PricePoint>,
        B: Fn(&Price, &Amount) -> ClosureFuture<QuantityPoint>,
        S: Fn(&Price, &Quantity) -> ClosureFuture<AmountPoint>;
}

#[cfg(test)]
mod tests_limit_position {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;
    use tracing_test::traced_test;

    fn decimal(value: f64) -> Decimal {
        Decimal::from_f64(value).unwrap()
    }

    #[tokio::test]
    #[traced_test]
    async fn test_predictive_buying() {
        let limit_position =
            LimitPosition::new(decimal(50.0), decimal(100.0), decimal(150.0), None);
        assert_eq!(limit_position.predictive_buying(&decimal(160.0)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(150.0)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(125.0)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(100.0)), None);
        assert_eq!(
            limit_position.predictive_buying(&decimal(99.99)),
            Some(decimal(50.0))
        );
        assert_eq!(
            limit_position.predictive_buying(&decimal(60.95)),
            Some(decimal(50.0))
        );

        let limit_position = LimitPosition::new(
            decimal(50.0),
            decimal(100.0),
            decimal(150.0),
            Some(decimal(2.0)),
        );
        assert_eq!(limit_position.predictive_buying(&decimal(125.0)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(100.0)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(99.99)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(60.95)), None);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_predictive_selling() {
        let limit_position =
            LimitPosition::new(decimal(50.0), decimal(100.0), decimal(150.0), None);
        assert_eq!(limit_position.predictive_selling(&decimal(160.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(125.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(100.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(60.95)), None);

        let limit_position = LimitPosition::new(
            decimal(50.0),
            decimal(100.0),
            decimal(150.0),
            Some(decimal(2.0)),
        );
        assert_eq!(
            limit_position.predictive_selling(&decimal(160.0)),
            Some(decimal(2.0))
        );
        assert_eq!(limit_position.predictive_selling(&decimal(150.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(125.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(100.0)), None);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_update_position() {
        let limit_position =
            LimitPosition::new(decimal(50.0), decimal(100.0), decimal(150.0), None);

        {
            limit_position.update_position(Some(decimal(50.0)));
            let position = limit_position.position.lock().unwrap();
            assert_eq!(*position, Some(decimal(50.0)));
        }

        {
            limit_position.update_position(Some(decimal(25.0)));
            let position = limit_position.position.lock().unwrap();
            assert_eq!(*position, Some(decimal(25.0)));
        }

        {
            limit_position.update_position(None);
            let position = limit_position.position.lock().unwrap();
            assert_eq!(*position, None);
        }
    }
}

#[cfg(test)]
mod tests_limit_trap {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;
    use tracing::info;
    use tracing_test::traced_test;

    fn decimal(value: f64) -> Decimal {
        Decimal::from_f64(value).unwrap()
    }

    #[derive(Debug, Default)]
    struct Buying {
        prices: Vec<Price>,
        quantitys: Vec<Quantity>,
        spents: Vec<Amount>,
        count: AtomicUsize,
    }

    #[derive(Debug, Default)]
    struct Selling {
        prices: Vec<Price>,
        quantitys: Vec<Quantity>,
        incomes: Vec<Amount>,
        count: AtomicUsize,
    }

    #[instrument]
    fn simple_trading() -> (
        (
            impl Fn(&Price, &Amount) -> ClosureFuture<QuantityPoint>,
            Arc<Mutex<Buying>>,
        ),
        (
            impl Fn(&Price, &Amount) -> ClosureFuture<AmountPoint>,
            Arc<Mutex<Selling>>,
        ),
    ) {
        let buying_information = Arc::new(Mutex::new(Buying::default()));
        let buying = buying_information.clone();
        let buy = move |price: &Price, amount: &Amount| -> ClosureFuture<QuantityPoint> {
            let quantity = amount / price;
            {
                let mut buying = buying.lock().unwrap();
                buying.count.fetch_add(1, Ordering::SeqCst);
                buying.prices.push(price.clone());
                buying.spents.push(amount.clone());
                buying.quantitys.push(quantity.clone());
                info!("Buying: {:?}", buying);
            }

            let f = async move { Ok(QuantityPoint::new(quantity)) };

            Box::pin(f)
        };

        let selling_information = Arc::new(Mutex::new(Selling::default()));
        let selling = selling_information.clone();
        let sell = move |price: &Price, quantity: &Quantity| -> ClosureFuture<AmountPoint> {
            let income = quantity / price;
            {
                let mut selling = selling.lock().unwrap();
                selling.count.fetch_add(1, Ordering::SeqCst);
                selling.prices.push(price.clone());
                selling.incomes.push(income.clone());
                selling.quantitys.push(quantity.clone());
                info!("Selling: {:?}", selling);
            }
            let f = async move { Ok(AmountPoint::new(income)) };

            Box::pin(f)
        };

        ((buy, buying_information), (sell, selling_information))
    }

    #[tokio::test]
    #[traced_test]
    async fn test_trap() {
        let limit_position = LimitPosition::new(
            decimal(50.0),
            decimal(100.0),
            decimal(200.0),
            Some(decimal(2.5)),
        );
        let limit = Limit::with_positions(vec![limit_position]);

        let prices = vec![
            decimal(210.0),
            decimal(200.0),
            decimal(150.0),
            decimal(100.0),
            decimal(90.50),
        ];
        let prices_len = prices.len();
        let p = Mutex::new(prices.into_iter());

        let price = || -> ClosureFuture<PricePoint> {
            let a = p.lock().unwrap().next().unwrap().clone();
            let f = async move { Ok(PricePoint::new(a)) };

            Box::pin(f)
        };

        let ((buy, buying), (sell, selling)) = simple_trading();
        
        for _ in 0..prices_len {
            limit.trap(&price, &buy, &sell).await.unwrap();
        }
        

        let buying = buying.lock().unwrap();
        assert_eq!(buying.count.load(Ordering::SeqCst), 0);

        let selling = selling.lock().unwrap();
        assert_eq!(selling.count.load(Ordering::SeqCst), 1);
        assert_eq!(selling.quantitys, vec![decimal(2.5)]);

    }
}

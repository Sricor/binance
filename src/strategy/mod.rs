pub mod grid;
pub mod limit;
// mod percentage;

use std::{error::Error, future::Future, pin::Pin, sync::Arc};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::noun::*;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Range(pub Decimal, pub Decimal);

impl Range {
    pub fn is_within_inclusive(&self, value: &Decimal) -> bool {
        value >= &self.low() && value <= &self.high()
    }

    pub fn is_within_exclusive(&self, value: &Decimal) -> bool {
        value > &self.low() && value < &self.high()
    }

    pub fn high(&self) -> &Decimal {
        if self.0 > self.1 {
            return &self.0;
        }

        &self.1
    }

    pub fn low(&self) -> &Decimal {
        if self.0 < self.1 {
            return &self.0;
        }

        &self.1
    }

    pub fn length(&self) -> Decimal {
        self.high() - self.low()
    }
}

pub type ClosureFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, Box<dyn Error + Send + Sync>>> + Send + Sync>>;

pub trait Strategy {
    fn trap<P, B, S>(
        &self,
        price: &P,
        buy: &B,
        sell: &S,
    ) -> impl Future<Output = Result<(), Box<dyn Error + Send + Sync>>>
    where
        P: Fn() -> ClosureFuture<PricePoint>,
        B: Fn(Price, Amount) -> ClosureFuture<QuantityPoint>,
        S: Fn(Price, Quantity) -> ClosureFuture<AmountPoint>;
}

pub trait Exchanger {
    fn spawn_price(self: &Arc<Self>) -> impl Fn() -> ClosureFuture<PricePoint>;
    fn spawn_buy(self: &Arc<Self>) -> impl Fn(Price, Amount) -> ClosureFuture<QuantityPoint>;
    fn spawn_sell(self: &Arc<Self>) -> impl Fn(Price, Quantity) -> ClosureFuture<AmountPoint>;
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PricePoint {
    value: Price,
    timestamp: i64,
}

impl PricePoint {
    pub fn new(price: Price) -> Self {
        Self {
            value: price,
            timestamp: timestamp_millis(),
        }
    }

    pub fn value(&self) -> &Price {
        &self.value
    }

    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AmountPoint {
    value: Amount,
    timestamp: i64,
}

impl AmountPoint {
    pub fn new(amount: Amount) -> Self {
        Self {
            value: amount,
            timestamp: timestamp_millis(),
        }
    }

    pub fn value(&self) -> &Amount {
        &self.value
    }

    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct QuantityPoint {
    value: Quantity,
    timestamp: i64,
}

impl QuantityPoint {
    pub fn new(quantity: Quantity) -> Self {
        Self {
            value: quantity,
            timestamp: timestamp_millis(),
        }
    }

    pub fn value(&self) -> &Quantity {
        &self.value
    }

    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

fn timestamp_millis() -> i64 {
    let now = Utc::now();

    now.timestamp_millis()
}

#[cfg(test)]
mod tests_range {
    use super::*;
    use tests_general::*;

    #[test]
    fn test_is_is_within_inclusive() {
        assert_eq!(
            Range(decimal(60.0), decimal(80.0)).is_within_inclusive(&decimal(70.0)),
            true
        );
        assert_eq!(
            Range(decimal(71880.0), decimal(72000.0)).is_within_inclusive(&decimal(72000.0)),
            true
        );
    }
}

#[cfg(test)]
pub(crate) mod tests_general {
    use std::borrow::BorrowMut;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard};

    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;
    pub(super) use tracing::debug;
    pub(super) use tracing_test::traced_test;

    use super::*;

    pub(super) fn decimal(value: f64) -> Decimal {
        Decimal::from_f64(value).unwrap()
    }

    pub(super) fn range(left: f64, right: f64) -> Range {
        Range(decimal(left), decimal(right))
    }

    #[derive(Debug, Default)]
    pub(super) struct Buying {
        pub(super) prices: Vec<Price>,
        pub(super) quantitys: Vec<Quantity>,
        pub(super) amounts: Vec<Amount>,
        pub(super) count: AtomicUsize,
    }

    #[derive(Debug, Default)]
    pub(super) struct Selling {
        pub(super) prices: Vec<Price>,
        pub(super) quantitys: Vec<Quantity>,
        pub(super) incomes: Vec<Amount>,
        pub(super) count: AtomicUsize,
    }

    pub(super) struct Trading {
        pub(super) buy: Box<dyn Fn(Price, Amount) -> ClosureFuture<QuantityPoint>>,
        pub(super) sell: Box<dyn Fn(Price, Quantity) -> ClosureFuture<AmountPoint>>,
        pub(super) buying: Arc<Mutex<Buying>>,
        pub(super) selling: Arc<Mutex<Selling>>,
    }

    impl Trading {
        pub(super) fn buying(&self) -> MutexGuard<Buying> {
            self.buying.lock().unwrap()
        }

        pub(super) fn selling(&self) -> MutexGuard<Selling> {
            self.selling.lock().unwrap()
        }
    }

    pub(super) fn simple_trading() -> Trading {
        let buying_information = Arc::new(Mutex::new(Buying::default()));
        let buying = buying_information.clone();
        let buy = move |price: Price, amount: Amount| -> ClosureFuture<QuantityPoint> {
            let quantity = (amount / price).trunc_with_scale(5);
            {
                let mut buying = buying.lock().unwrap();
                buying.count.fetch_add(1, Ordering::SeqCst);
                buying.prices.push(price.clone());
                buying.amounts.push(amount.clone());
                buying.quantitys.push(quantity.clone());
                debug!("Buying: {:?}", buying);
            }

            let f = async move { Ok(QuantityPoint::new(quantity)) };

            Box::pin(f)
        };

        let selling_information = Arc::new(Mutex::new(Selling::default()));
        let selling = selling_information.clone();
        let sell = move |price: Price, quantity: Quantity| -> ClosureFuture<AmountPoint> {
            let income = (quantity / price).trunc_with_scale(5);
            {
                let mut selling = selling.lock().unwrap();
                selling.count.fetch_add(1, Ordering::SeqCst);
                selling.prices.push(price.clone());
                selling.incomes.push(income.clone());
                selling.quantitys.push(quantity.clone());
                debug!("Selling: {:?}", selling);
            }
            let f = async move { Ok(AmountPoint::new(income)) };

            Box::pin(f)
        };

        let result = Trading {
            buy: Box::new(buy),
            sell: Box::new(sell),
            buying: buying_information,
            selling: selling_information,
        };

        result
    }

    pub(crate) fn simple_prices(prices: Vec<f64>) -> impl Fn() -> ClosureFuture<PricePoint> {
        let iter = Mutex::new(prices.into_iter());
        let price = move || -> ClosureFuture<PricePoint> {
            let item = iter.lock().unwrap().borrow_mut().next().unwrap();

            let f = async move { Ok(PricePoint::new(decimal(item))) };

            Box::pin(f)
        };

        price
    }
}

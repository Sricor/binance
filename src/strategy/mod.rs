mod grid;
pub mod limit;
mod percentage;

pub mod strategy {
    pub use super::grid::{Bound, BoundPosition, Grid};
    pub use super::percentage::Percentage;
}

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{error::Error, future::Future, pin::Pin};

use crate::{common::time::timestamp_millis, noun::*};

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

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PriceSignal {
    value: Price,
    timestamp: i64,
}

impl PriceSignal {
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
pub struct Order {
    price: Price,
    amount: Amount,
    quantity: Quantity,
    timestamp: i64,
}

impl Order {
    pub fn new(price: Price, amount: Amount, quantity: Quantity) -> Self {
        Self {
            price,
            amount,
            quantity,
            timestamp: Utc::now().timestamp(),
        }
    }

    pub fn price(&self) -> &Price {
        &self.price
    }

    pub fn amount(&self) -> &Amount {
        &self.amount
    }

    pub fn quantity(&self) -> &Quantity {
        &self.quantity
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Position {
    Stock(Order),
    None,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Positions<T> {
    Stock(T),
    None,
}

impl Position {
    pub fn is_none(&self) -> bool {
        match &self {
            Self::None => true,

            _ => false,
        }
    }

    pub fn is_stock(&self) -> bool {
        match &self {
            Self::Stock(_) => true,

            _ => false,
        }
    }
}

impl<T> Positions<T> {
    pub fn is_none(&self) -> bool {
        match &self {
            Self::None => true,

            _ => false,
        }
    }

    pub fn is_stock(&self) -> bool {
        match &self {
            Self::Stock(_) => true,

            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum PositionSide {
    /// Complete buying order
    Increase(Order),

    /// Complete selling order
    Decrease(Order),
}

pub type ClosureFuture<T> = Pin<Box<dyn Future<Output = Result<T, Box<dyn Error>>> + Send>>;

pub trait Strategy {
    fn trap<P, B, S>(
        &self,
        price: &P,
        buy: &B,
        sell: &S,
    ) -> impl Future<Output = Result<(), Box<dyn Error>>>
    where
        P: Fn() -> ClosureFuture<PricePoint>,
        B: Fn(&Price, &Amount) -> ClosureFuture<QuantityPoint>,
        S: Fn(&Price, &Quantity) -> ClosureFuture<AmountPoint>;
}

pub trait Master {
    type Item;

    fn trap<S, T>(
        &self,
        price: &PriceSignal,
        strategy: &S,
        treasurer: Option<&T>,
    ) -> impl Future<Output = Result<Self::Item, impl Error>> + Send
    where
        S: Strategy + Send + Sync,
        T: Treasurer + Send + Sync;
}

pub trait Treasurer {
    fn balance(&self) -> impl Future<Output = Decimal> + Send;

    // income
    fn transfer_in(&self, amount: &Amount) -> impl Future<Output = ()> + Send;

    // spent
    fn transfer_out(&self, amount: &Amount) -> impl Future<Output = ()> + Send;
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

#[cfg(test)]
mod tests_general {
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
        pub(super) amount: Vec<Amount>,
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
        pub(super) buy: Box<dyn Fn(&Price, &Amount) -> ClosureFuture<QuantityPoint>>,
        pub(super) sell: Box<dyn Fn(&Price, &Amount) -> ClosureFuture<AmountPoint>>,
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
        let buy = move |price: &Price, amount: &Amount| -> ClosureFuture<QuantityPoint> {
            let quantity = (amount / price).trunc_with_scale(5);
            {
                let mut buying = buying.lock().unwrap();
                buying.count.fetch_add(1, Ordering::SeqCst);
                buying.prices.push(price.clone());
                buying.amount.push(amount.clone());
                buying.quantitys.push(quantity.clone());
                debug!("Buying: {:?}", buying);
            }

            let f = async move { Ok(QuantityPoint::new(quantity)) };

            Box::pin(f)
        };

        let selling_information = Arc::new(Mutex::new(Selling::default()));
        let selling = selling_information.clone();
        let sell = move |price: &Price, quantity: &Quantity| -> ClosureFuture<AmountPoint> {
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

    pub(super) fn simple_prices(prices: Vec<f64>) -> impl Fn() -> ClosureFuture<PricePoint> {
        let iter = Mutex::new(prices.into_iter());
        let price = move || -> ClosureFuture<PricePoint> {
            let item = iter.lock().unwrap().borrow_mut().next().unwrap();

            let f = async move { Ok(PricePoint::new(decimal(item))) };

            Box::pin(f)
        };

        price
    }
}

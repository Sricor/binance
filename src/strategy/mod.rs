mod grid;
mod limit;
mod percentage;

pub mod strategy {
    pub use super::grid::{Bound, BoundPosition, Grid};
    pub use super::percentage::Percentage;
}

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{error::Error, future::Future};

use crate::{common::time::timestamp_millis, noun::*};

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

pub trait Strategy {
    // Buy signal, return Some (Amount) when buying is required
    fn predictive_buying(&self, price: &PriceSignal)
        -> impl Future<Output = Option<Amount>> + Send;

    // Sell signal, return Some (Vec<Position>) when selling is required
    fn predictive_selling(
        &self,
        price: &PriceSignal,
    ) -> impl Future<Output = Option<Vec<Order>>> + Send;

    // update strategic positions after passing a trade
    fn update_position(&self, side: &PositionSide) -> impl Future<Output = ()> + Send;

    fn is_completed(&self) -> bool;
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

mod grid;
mod percentage;

pub mod strategy {
    pub use super::grid::{Bound, BoundPosition, Grid};
    pub use super::percentage::Percentage;
}

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{error::Error, future::Future};

use crate::noun::*;

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
pub enum PositionSide {
    /// Complete buying order
    Increase(Order),

    /// Complete selling order
    Decrease(Order),
}

pub trait Strategy {
    // Buy signal, return Some (Amount) when buying is required
    fn predictive_buying(&self, price: &Price) -> impl Future<Output = Option<Amount>> + Send;

    // Sell signal, return Some (Vec<Position>) when selling is required
    fn predictive_selling(&self, price: &Price) -> impl Future<Output = Option<Vec<Order>>> + Send;

    // update strategic positions after passing a trade
    fn update_position(&self, side: &PositionSide) -> impl Future<Output = ()> + Send;

    fn is_completed(&self) -> bool;
}

pub trait Master {
    type Item;

    fn trap<S, T>(
        &self,
        price: &Price,
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

mod percentage;

pub mod strategy {
    pub use super::percentage::Percentage;
}

use std::future::Future;

use crate::noun::*;

#[derive(Clone, PartialEq)]
pub struct Position {
    price: Price,
    amount: Amount,
    quantity: Quantity,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            price: Decimal::ZERO,
            amount: Decimal::ZERO,
            quantity: Decimal::ZERO,
        }
    }
}

impl Position {
    pub fn new(price: Price, amount: Amount, quantity: Quantity) -> Self {
        Self {
            price,
            amount,
            quantity,
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

pub enum PositionSide {
    Increase(Position),
    Decrease(Position),
}

pub trait Strategy {
    // Buy signal, return Some (Amount) when buying is required
    fn predictive_buy(&self, price: &Price) -> impl Future<Output = Option<Amount>> + Send;

    // Sell signal, return Some (Vec<Position>) when selling is required
    fn predictive_sell(&self, price: &Price) -> impl Future<Output = Option<Vec<Position>>> + Send;

    // update strategic positions after passing a trade
    fn update_position(&self, side: &PositionSide) -> impl Future<Output = ()> + Send;

    fn is_completed(&self) -> bool;
}

pub trait Master {
    fn trap(
        &self,
        price: &Price,
        strategy: &(impl Strategy + Send + Sync),
        treasurer: &(impl Treasurer + Send + Sync),
    ) -> impl std::future::Future<Output = Result<(), impl std::error::Error>> + Send;
}

pub trait Treasurer {
    fn balance(&self) -> impl std::future::Future<Output = Decimal> + Send;

    // incom
    fn transfer_in(&self, amount: &Amount) -> impl std::future::Future<Output = ()> + Send;

    // spent
    fn transfer_out(&self, amount: &Amount) -> impl std::future::Future<Output = ()> + Send;
}

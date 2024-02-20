use std::sync::Mutex;

use tracing::instrument;

use super::{Amount, Order, Position, Price, PriceSignal, Strategy};

pub struct LimitPosition {
    pub buying: Price,
    pub selling: Price,
    pub investment: Amount,
    pub position: Mutex<Position>,
}

impl LimitPosition {
    pub fn new(
        investment: Amount,
        buying: Price,
        selling: Price,
        position: Option<Position>,
    ) -> Self {
        Self {
            investment,
            buying,
            selling,
            position: Mutex::new(position.unwrap_or_default()),
        }
    }
}

pub struct Limit {
    positions: Vec<LimitPosition>,
}

impl Limit {
    pub fn with_positions(positions: Vec<LimitPosition>) -> Self {
        Self { positions }
    }

    pub fn insert_position(&mut self, index: usize, position: LimitPosition) {
        self.positions.insert(index, position)
    }
}


// impl Strategy for Limit {
//     #[instrument(skip(self))]
//     async fn predictive_buying(&self, price: &PriceSignal) -> Option<Amount> {
//     }

//     #[instrument(skip(self))]
//     async fn predictive_selling(&self, price: &PriceSignal) -> Option<Vec<Order>> {
//     }

//     #[instrument(skip(self))]
//     async fn update_position(&self, side: &PositionSide) -> () {
//     }

//     fn is_completed(&self) -> bool {
//         false
//     }
// }
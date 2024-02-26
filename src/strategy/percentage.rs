use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Mutex;

use super::{Order, PositionSide, PriceSignal, Strategy};
use crate::noun::*;

pub struct Percentage {
    investment: Amount,
    target_percent: Decimal,
    is_completed: AtomicBool,
    stop_percent: Option<Decimal>,
    positions: Mutex<Vec<Order>>,
    start_buying_price: Option<Price>,
}

impl Percentage {
    pub fn new(
        investment: Amount,
        target_percent: Decimal,
        stop_percent: Option<Decimal>,
        start_buying_price: Option<Price>,
    ) -> Self {
        Percentage {
            investment,
            target_percent,
            stop_percent,
            start_buying_price,
            is_completed: AtomicBool::new(false),
            positions: Mutex::new(Vec::with_capacity(2)),
        }
    }

    fn completed(&self) {
        self.is_completed.store(true, Ordering::SeqCst)
    }

    pub async fn positions(&self) -> Vec<Order> {
        self.positions.lock().await.clone()
    }

    async fn predictive_buying(&self, price: &PriceSignal) -> Option<Amount> {
        if self.is_completed() {
            return None;
        }

        if let Some(start_price) = self.start_buying_price {
            if price.value() < &start_price {
                return None;
            }
        }

        if self.positions.lock().await.is_empty() {
            return Some(self.investment);
        }

        None
    }

    async fn predictive_selling(&self, price: &PriceSignal) -> Option<Vec<Order>> {
        if self.is_completed() {
            return None;
        }

        let position = self.positions.lock().await;
        let result = position
            .iter()
            .filter_map(|e| {
                if price.value() > &(e.price * (Decimal::ONE + self.target_percent)) {
                    return Some(e.clone());
                };

                if let Some(stop_loss_percent) = self.stop_percent {
                    if price.value() < &(e.price * (Decimal::ONE + stop_loss_percent)) {
                        return Some(e.clone());
                    }
                }

                None
            })
            .collect();

        Some(result)
    }

    async fn update_position(&self, side: &PositionSide) {
        let mut positions = self.positions.lock().await;
        match side {
            PositionSide::Increase(v) => positions.push(v.clone()),
            PositionSide::Decrease(v) => {
                if let Some(index) = positions.iter().position(|e| e == v) {
                    println!("completed");
                    positions.remove(index);
                    self.completed();
                };
            }
        };
    }

    fn is_completed(&self) -> bool {
        self.is_completed.load(Ordering::SeqCst)
    }
}

// impl Strategy for Percentage {

// }

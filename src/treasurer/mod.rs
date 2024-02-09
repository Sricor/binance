use tokio::sync::Mutex;

use crate::noun::*;
use crate::strategy::Treasurer;

pub struct Prosperity {
    balance: Mutex<Decimal>,
}

impl Prosperity {
    pub fn new(balance: Option<Decimal>) -> Self {
        Self {
            balance: Mutex::new(balance.unwrap_or(Decimal::ZERO)),
        }
    }
}

impl Treasurer for Prosperity {
    async fn transfer_in(&self, amount: &crate::noun::Amount) {
        let mut balance = self.balance.lock().await;
        *balance = *balance + amount
    }

    async fn transfer_out(&self, amount: &crate::noun::Amount) {
        let mut balance = self.balance.lock().await;
        *balance = *balance - amount
    }

    async fn balance(&self) -> Decimal {
        self.balance.lock().await.clone()
    }
}

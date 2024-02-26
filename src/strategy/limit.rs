use std::error::Error;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{
    Amount, AmountPoint, ClosureFuture, Price, PricePoint, Quantity, QuantityPoint, Range, Strategy,
};

pub type Position = Option<Quantity>;

#[derive(Debug, Serialize, Deserialize)]
pub struct LimitPosition {
    pub buying: Range,
    pub selling: Range,
    pub investment: Amount,
    pub position: Mutex<Position>,
}

impl LimitPosition {
    pub fn new(investment: Amount, buying: Range, selling: Range, position: Position) -> Self {
        Self {
            investment,
            buying,
            selling,
            position: Mutex::new(position),
        }
    }

    fn predictive_buying(&self, price: &Price) -> Option<Amount> {
        if self.buying.is_within_inclusive(price) {
            let position = self.position.lock().unwrap();

            if let None = *position {
                return Some(self.investment);
            }
        }

        None
    }

    fn predictive_selling(&self, price: &Price) -> Option<Quantity> {
        if self.selling.is_within_inclusive(price) {
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

impl Strategy for Limit {
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

#[cfg(test)]
mod tests_limit_position {
    use super::super::tests_general::*;
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_predictive_buying() {
        let limit_position =
            LimitPosition::new(decimal(50.0), range(0.0, 100.0), range(150.0, 300.0), None);
        assert_eq!(limit_position.predictive_buying(&decimal(160.0)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(150.0)), None);
        assert_eq!(limit_position.predictive_buying(&decimal(125.0)), None);
        assert_eq!(
            limit_position.predictive_buying(&decimal(100.0)),
            Some(decimal(50.0))
        );
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
            range(0.0, 100.0),
            range(150.0, 300.0),
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
            LimitPosition::new(decimal(50.0), range(0.0, 100.0), range(150.0, 300.0), None);
        assert_eq!(limit_position.predictive_selling(&decimal(160.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(125.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(100.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(60.95)), None);

        let limit_position = LimitPosition::new(
            decimal(50.0),
            range(0.0, 100.0),
            range(150.0, 300.0),
            Some(decimal(2.0)),
        );
        assert_eq!(
            limit_position.predictive_selling(&decimal(160.0)),
            Some(decimal(2.0))
        );
        assert_eq!(
            limit_position.predictive_selling(&decimal(150.0)),
            Some(decimal(2.0))
        );
        assert_eq!(limit_position.predictive_selling(&decimal(125.0)), None);
        assert_eq!(limit_position.predictive_selling(&decimal(100.0)), None);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_update_position() {
        let limit_position =
            LimitPosition::new(decimal(50.0), range(0.0, 100.0), range(150.0, 300.0), None);

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
    use std::sync::atomic::Ordering;

    use tracing_test::traced_test;

    use super::super::tests_general::*;
    use super::*;

    /// ### Limit Position          
    /// Investment Amount:   50.0   
    /// Buying     Price:    0.0   - 100.0  
    /// Selling    Price:    200.0 - 300.0  
    /// Position   Quantity: None    
    fn single_none_position() -> Limit {
        let limit_position =
            LimitPosition::new(decimal(50.0), range(0.0, 100.0), range(200.0, 300.0), None);
        let result = Limit::with_positions(vec![limit_position]);

        result
    }

    /// ### Limit Position          
    /// Investment Amount:   50.0   
    /// Buying     Price:    0.0   - 100.0  
    /// Selling    Price:    200.0 - 300.0  
    /// Position   Quantity: 2.5    
    fn single_some_position() -> Limit {
        let limit_position = LimitPosition::new(
            decimal(50.0),
            range(0.0, 100.0),
            range(200.0, 300.0),
            Some(decimal(2.5)),
        );
        let result = Limit::with_positions(vec![limit_position]);

        result
    }

    #[tokio::test]
    #[traced_test]
    async fn test_trap_single_some_position() {
        let limit = single_some_position();
        let prices = vec![210.0, 200.0, 150.0, 100.0, 90.50];

        let trading = simple_trading();
        let price = simple_prices(prices.clone());

        for _ in 0..prices.len() {
            limit
                .trap(&price, &trading.buy, &trading.sell)
                .await
                .unwrap();
        }

        {
            let buying = trading.buying();
            let selling = trading.selling();

            assert_eq!(selling.quantitys, vec![decimal(2.5)]);
            assert_eq!(selling.count.load(Ordering::SeqCst), 1);

            assert_eq!(buying.amount, vec![decimal(50.0)]);
            assert_eq!(buying.count.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_trap_single_none_position() {
        let limit = single_none_position();

        let prices = vec![210.0, 200.0, 150.0, 100.0, 90.50];
        let trading = simple_trading();
        let price = simple_prices(prices.clone());

        for _ in 0..prices.len() {
            limit
                .trap(&price, &trading.buy, &trading.sell)
                .await
                .unwrap();
        }

        {
            let buying = trading.buying();
            let selling = trading.selling();

            assert_eq!(selling.quantitys, vec![]);
            assert_eq!(selling.count.load(Ordering::SeqCst), 0);

            assert_eq!(buying.amount, vec![decimal(50.0)]);
            assert_eq!(buying.count.load(Ordering::SeqCst), 1);
        }
    }
}

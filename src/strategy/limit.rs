use std::error::Error;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use rust_decimal::Decimal;
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

    buying_count: AtomicUsize,
    selling_count: AtomicUsize,
}

impl LimitPosition {
    pub fn new(investment: Amount, buying: Range, selling: Range, position: Position) -> Self {
        Self {
            investment,
            buying,
            buying_count: AtomicUsize::default(),
            selling,
            selling_count: AtomicUsize::default(),
            position: Mutex::new(position),
        }
    }

    pub fn selling_count(&self) -> usize {
        self.selling_count.load(Ordering::Relaxed)
    }

    pub fn buying_count(&self) -> usize {
        self.buying_count.load(Ordering::Relaxed)
    }

    pub fn predictive_buying(&self, price: &Price) -> Option<Amount> {
        if self.buying.is_within_inclusive(price) {
            let position = self.position.lock().ok()?;

            return match *position {
                Some(pos) => {
                    if pos == Decimal::ZERO {
                        Some(self.investment)
                    } else {
                        None
                    }
                }
                None => Some(self.investment),
            };
        }

        None
    }

    pub fn predictive_selling(&self, price: &Price) -> Option<Quantity> {
        if self.selling.is_within_inclusive(price) {
            let position = self.position.lock().ok()?;

            if let Some(quantity) = *position {
                if quantity == Decimal::ZERO {
                    return None;
                }

                return Some(quantity.clone());
            }
        }

        None
    }

    pub fn update_position(&self, position: Position) {
        let mut source = self.position.lock().unwrap();

        *source = position;
    }

    fn fetch_add_buying_count(&self, val: usize) {
        self.buying_count.fetch_add(val, Ordering::Relaxed);
    }

    fn fetch_add_selling_count(&self, val: usize) {
        self.selling_count.fetch_add(val, Ordering::Relaxed);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Limit {
    positions: Vec<LimitPosition>,
}

impl Limit {
    pub fn with_positions(positions: Vec<LimitPosition>) -> Self {
        Self { positions }
    }

    pub fn positions(&self) -> &Vec<LimitPosition> {
        &self.positions
    }
}

impl Strategy for Limit {
    #[instrument(skip_all)]
    async fn trap<P, B, S>(
        &self,
        price: &P,
        buy: &B,
        sell: &S,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        P: Fn() -> ClosureFuture<PricePoint>,
        B: Fn(Price, Amount) -> ClosureFuture<QuantityPoint>,
        S: Fn(Price, Quantity) -> ClosureFuture<AmountPoint>,
    {
        let price = match price().await {
            Ok(v) => v.value().clone(),
            Err(e) => return Err(e),
        };

        for limit_position in self.positions.iter() {
            if let Some(quantity) = limit_position.predictive_selling(&price) {
                let _ = sell(price, quantity).await?;
                limit_position.update_position(None);
                limit_position.fetch_add_selling_count(1);

                continue;
            }

            if let Some(amount) = limit_position.predictive_buying(&price) {
                let quantity = buy(price, amount).await?;
                limit_position.update_position(Some(quantity.value().clone()));
                limit_position.fetch_add_buying_count(1);

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
    /// - Investment Amount:   50.0   
    /// - Buying     Price:    0.0   - 100.0  
    /// - Selling    Price:    200.0 - 300.0  
    /// - Position   Quantity: None    
    fn single_none_position_limit() -> Limit {
        let limit_position =
            LimitPosition::new(decimal(50.0), range(0.0, 100.0), range(200.0, 300.0), None);
        let result = Limit::with_positions(vec![limit_position]);

        result
    }

    /// ### Limit Position          
    /// - Investment Amount:   50.0   
    /// - Buying     Price:    0.0   - 100.0  
    /// - Selling    Price:    200.0 - 300.0  
    /// - Position   Quantity: Some(0)    
    fn single_some_empty_position_limit() -> Limit {
        let limit_position = LimitPosition::new(
            decimal(50.0),
            range(0.0, 100.0),
            range(200.0, 300.0),
            Some(decimal(0.0)),
        );
        let result = Limit::with_positions(vec![limit_position]);

        result
    }

    /// ### Limit Position          
    /// - Investment Amount:   50.0   
    /// - Buying     Price:    0.0   - 100.0  
    /// - Selling    Price:    200.0 - 300.0  
    /// - Position   Quantity: 2.5    
    fn single_some_position_limit() -> Limit {
        let limit_position = LimitPosition::new(
            decimal(50.0),
            range(0.0, 100.0),
            range(200.0, 300.0),
            Some(decimal(2.5)),
        );
        let result = Limit::with_positions(vec![limit_position]);

        result
    }

    /// ### Limit Position One                            
    /// - Investment Amount:   10.0                       
    /// - Buying     Price:    0.0   - 50.0               
    /// - Selling    Price:    100.0 - 200.0              
    /// - Position   Quantity: None                       
    ///                                                   
    /// ### Limit Position Two                            
    /// - Investment Amount:   20.0                       
    /// - Buying     Price:    0.0   - 30.0               
    /// - Selling    Price:    120.0 - 200.0              
    /// - Position   Quantity: None                       
    ///                                                   
    /// ### Limit Position Three                          
    /// - Investment Amount:   30.0                       
    /// - Buying     Price:    0.0   - 80.0               
    /// - Selling    Price:    150.0 - 200.0              
    /// - Position   Quantity: None                       
    ///                                                   
    /// ### Limit Position Four                           
    /// - Investment Amount:   40.0                       
    /// - Buying     Price:    0.0   - 100.0              
    /// - Selling    Price:    150.0 - 200.0              
    /// - Position   Quantity: 5.0                        
    fn multi_position_limit() -> Limit {
        let limit_position_one =
            LimitPosition::new(decimal(10.0), range(0.0, 50.0), range(100.0, 200.0), None);
        let limit_position_two =
            LimitPosition::new(decimal(20.0), range(0.0, 30.0), range(120.0, 200.0), None);
        let limit_position_three =
            LimitPosition::new(decimal(30.0), range(0.0, 80.0), range(150.0, 200.0), None);
        let limit_position_four = LimitPosition::new(
            decimal(40.0),
            range(0.0, 100.0),
            range(150.0, 200.0),
            Some(decimal(5.0)),
        );

        let result = Limit::with_positions(vec![
            limit_position_one,
            limit_position_two,
            limit_position_three,
            limit_position_four,
        ]);

        result
    }

    #[tokio::test]
    #[traced_test]
    async fn test_trap_single_some_position() {
        let trading = simple_trading();
        let limit = single_some_position_limit();

        let prices = vec![210.0, 200.0, 150.0, 100.0, 90.50];
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
            assert_eq!(limit.positions[0].selling_count(), 1);

            assert_eq!(buying.amounts, vec![decimal(50.0)]);
            assert_eq!(buying.count.load(Ordering::SeqCst), 1);
            assert_eq!(limit.positions[0].buying_count(), 1);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_trap_single_none_position() {
        let trading = simple_trading();
        let limit = single_none_position_limit();

        let prices = vec![210.0, 200.0, 150.0, 100.0, 90.50];
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
            assert_eq!(limit.positions[0].selling_count(), 0);

            assert_eq!(buying.amounts, vec![decimal(50.0)]);
            assert_eq!(buying.count.load(Ordering::SeqCst), 1);
            assert_eq!(limit.positions[0].buying_count(), 1);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_trap_single_some_empty_position() {
        let trading = simple_trading();
        let limit = single_some_empty_position_limit();

        let prices = vec![210.0, 200.0, 150.0, 100.0, 90.50];
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
            assert_eq!(limit.positions[0].selling_count(), 0);

            assert_eq!(buying.amounts, vec![decimal(50.0)]);
            assert_eq!(buying.count.load(Ordering::SeqCst), 1);
            assert_eq!(limit.positions[0].buying_count(), 1);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_trap_mix() {
        let trading = simple_trading();
        let limit = multi_position_limit();

        let prices = vec![60.5, 30.0, 30.5, 35.5, 50.0, 110.5, 160.5, 15.0];
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

            assert_eq!(buying.count.load(Ordering::SeqCst), 7);
            assert_eq!(limit.positions[0].buying_count(), 2);
            assert_eq!(limit.positions[1].buying_count(), 2);
            assert_eq!(limit.positions[2].buying_count(), 2);
            assert_eq!(limit.positions[3].buying_count(), 1);
            assert_eq!(
                buying.prices,
                vec![
                    decimal(60.5),
                    decimal(30.0),
                    decimal(30.0),
                    decimal(15.0),
                    decimal(15.0),
                    decimal(15.0),
                    decimal(15.0)
                ]
            );
            assert_eq!(
                buying.amounts,
                vec![
                    decimal(30.0),
                    decimal(10.0),
                    decimal(20.0),
                    decimal(10.0),
                    decimal(20.0),
                    decimal(30.0),
                    decimal(40.0)
                ]
            );

            assert_eq!(selling.count.load(Ordering::SeqCst), 4);
            assert_eq!(limit.positions[0].selling_count(), 1);
            assert_eq!(limit.positions[1].selling_count(), 1);
            assert_eq!(limit.positions[2].selling_count(), 1);
            assert_eq!(limit.positions[3].selling_count(), 1);
            assert_eq!(
                selling.prices,
                vec![
                    decimal(110.5),
                    decimal(160.5),
                    decimal(160.5),
                    decimal(160.5)
                ]
            );
        }
    }
}

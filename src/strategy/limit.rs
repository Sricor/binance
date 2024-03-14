use std::error::Error;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::extension::LockResultExt;

use super::{
    Amount, AmountPoint, PinFutureResult, Price, PricePoint, Quantity, QuantityPoint, Range,
    Strategy,
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
}

// ===== Limit Position Trading =====
impl LimitPosition {
    pub fn is_short(&self) -> bool {
        let position = &*self.position.lock().ignore_poison();
        match Self::position_quantity(position) {
            Some(_quantity) => false,
            None => true,
        }
    }

    fn position_quantity(position: &Position) -> Option<&Quantity> {
        match position {
            Some(quantity) => {
                if quantity == &Decimal::ZERO {
                    None
                } else {
                    Some(quantity)
                }
            }
            None => None,
        }
    }

    async fn buy<B>(
        &self,
        f: B,
        price: Price,
    ) -> Result<QuantityPoint, Box<dyn Error + Send + Sync>>
    where
        B: Fn(Price, Amount) -> PinFutureResult<QuantityPoint>,
    {
        let result = {
            let mut position = self.position.lock().ignore_poison();

            match Self::position_quantity(&*position) {
                Some(_quantity) => return Err("current position is already held".into()),
                None => {
                    let quantity_point = f(price, self.investment).await?;
                    *position = Some(quantity_point.value().clone());

                    quantity_point
                }
            }
        };

        self.fetch_add_buying_count(1);

        Ok(result)
    }

    async fn sell<S>(&self, f: S, price: Price) -> Result<AmountPoint, Box<dyn Error + Send + Sync>>
    where
        S: Fn(Price, Quantity) -> PinFutureResult<AmountPoint>,
    {
        let result = {
            let mut position = self.position.lock().ignore_poison();

            match Self::position_quantity(&*position) {
                None => return Err("no position quantity currently held".into()),
                Some(quantity) => {
                    let amount_point = f(price, quantity.clone()).await?;
                    *position = None;

                    amount_point
                }
            }
        };

        self.fetch_add_selling_count(1);

        Ok(result)
    }

    fn fetch_add_buying_count(&self, val: usize) {
        self.buying_count.fetch_add(val, Ordering::Relaxed);
    }

    fn fetch_add_selling_count(&self, val: usize) {
        self.selling_count.fetch_add(val, Ordering::Relaxed);
    }
}

impl Strategy for LimitPosition {
    #[instrument(skip_all)]
    async fn trap<P, B, S>(
        &self,
        price: &P,
        buy: &B,
        sell: &S,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        P: Fn() -> PinFutureResult<PricePoint>,
        B: Fn(Price, Amount) -> PinFutureResult<QuantityPoint>,
        S: Fn(Price, Quantity) -> PinFutureResult<AmountPoint>,
    {
        let price = price().await?.value().clone();

        if self.selling.is_within_inclusive(&price) {
            if !self.is_short() {
                self.sell(sell, price).await?;
            }
        }

        if self.buying.is_within_inclusive(&price) {
            if self.is_short() {
                self.buy(buy, price).await?;
            }
        }

        Ok(())
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
        P: Fn() -> PinFutureResult<PricePoint>,
        B: Fn(Price, Amount) -> PinFutureResult<QuantityPoint>,
        S: Fn(Price, Quantity) -> PinFutureResult<AmountPoint>,
    {
        let price = Self::spawn_price(price().await?);

        for position in self.positions.iter() {
            position.trap(&price, buy, sell).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests_limit_trap {
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

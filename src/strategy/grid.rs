use std::error::Error;

use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Serialize};

use super::{
    limit::{Limit, LimitPosition},
    AmountPoint, ClosureFuture, PricePoint, QuantityPoint, Range, Strategy,
};
use crate::noun::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Grid {
    limit: Limit,
    options: GridOptions,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GridOptions {
    pub stop_loss: Option<Decimal>,
}

impl Grid {
    pub fn new(
        investment: Amount,
        range: Range,
        copies: usize,
        options: Option<GridOptions>,
    ) -> Self {
        let limit = Limit::with_positions(Self::split(investment, range, copies));

        Self {
            limit,
            options: options.unwrap_or_default(),
        }
    }

    fn split(investment: Amount, range: Range, copies: usize) -> Vec<LimitPosition> {
        let mut result = Vec::with_capacity(copies);
        let investment = investment / Decimal::from(copies);
        let interval = (range.high() - range.low()) / Decimal::from(copies);
        let interval = interval.trunc_with_scale(6);

        for i in 0..copies - 1 {
            let buying = range.low() + interval * Decimal::from(i);
            let selling = range.low() + interval * Decimal::from(i + 2);
            result.push(LimitPosition::new(
                investment,
                Range(buying, buying + (interval / Decimal::TWO)),
                Range(selling - (interval / Decimal::TWO), selling),
                None,
            ))
        }

        result
    }

    pub fn predictive_lowest_profit_price(&self) -> Vec<Price> {
        let positions = self.limit.positions();
        let mut result = Vec::with_capacity(positions.len() + 1);

        for i in positions.iter() {
            let buying_price = i.buying.1 * Decimal::from_f64(0.9999).unwrap();
            let selling_price = i.selling.0 * Decimal::from_f64(1.0001).unwrap();
            result.push(buying_price.trunc_with_scale(8));
            result.push(selling_price.trunc_with_scale(8));
        }

        result
    }

    pub fn predictive_highest_profit_price(&self) -> Vec<Price> {
        let positions = self.limit.positions();
        let mut result = Vec::with_capacity(positions.len() + 1);

        for i in positions.iter() {
            let buying_price = i.buying.0 * Decimal::from_f64(1.0001).unwrap();
            let selling_price = i.selling.1 * Decimal::from_f64(0.9999).unwrap();
            result.push(buying_price.trunc_with_scale(8));
            result.push(selling_price.trunc_with_scale(8));
        }

        result
    }
}

impl Strategy for Grid {
    async fn trap<P, B, S>(&self, price: &P, buy: &B, sell: &S) -> Result<(), Box<dyn Error>>
    where
        P: Fn() -> ClosureFuture<PricePoint>,
        B: Fn(&Price, &Amount) -> ClosureFuture<QuantityPoint>,
        S: Fn(&Price, &Quantity) -> ClosureFuture<AmountPoint>,
    {
        self.limit.trap(price, buy, sell).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //     impl PartialEq for BoundPosition {
    //         fn eq(&self, other: &Self) -> bool {
    //             self.buying == other.buying && self.selling == other.selling
    //         }
    //     }

    //     use rust_decimal::prelude::FromPrimitive;
    //     use tracing_test::traced_test;

    //     use super::*;

    //     fn price(value: f64) -> PriceSignal {
    //         PriceSignal::new(decimal(value))
    //     }

    //     fn decimal(value: f64) -> Decimal {
    //         Decimal::from_f64(value).unwrap()
    //     }

    //     #[test]
    //     fn test_bound_position_with_copies_one() {
    //         let bound = BoundPosition::with_copies(Bound(decimal(50.0), decimal(90.0)), 4);
    //         let target = vec![
    //             BoundPosition {
    //                 buying: Bound(decimal(50.0), decimal(55.0)),
    //                 selling: Bound(decimal(65.0), decimal(70.0)),
    //                 position: Mutex::new(Position::None),
    //             },
    //             BoundPosition {
    //                 buying: Bound(decimal(60.0), decimal(65.0)),
    //                 selling: Bound(decimal(75.0), decimal(80.0)),
    //                 position: Mutex::new(Position::None),
    //             },
    //             BoundPosition {
    //                 buying: Bound(decimal(70.0), decimal(75.0)),
    //                 selling: Bound(decimal(85.0), decimal(90.0)),
    //                 position: Mutex::new(Position::None),
    //             },
    //         ];
    //         assert_eq!(bound, target);

    //         let bound = BoundPosition::with_copies(Bound(decimal(50.0), decimal(90.0)), 3);
    //         let target = vec![
    //             BoundPosition {
    //                 buying: Bound(decimal(50.0), decimal(56.66666650)),
    //                 selling: Bound(decimal(69.99999950), decimal(76.666666)),
    //                 position: Mutex::new(Position::None),
    //             },
    //             BoundPosition {
    //                 buying: Bound(decimal(63.333333), decimal(69.99999950)),
    //                 selling: Bound(decimal(83.33333250), decimal(89.999999)),
    //                 position: Mutex::new(Position::None),
    //             },
    //         ];
    //         assert_eq!(bound, target);
    //     }

    //     #[test]
    //     fn test_bound_position_with_copies_two() {
    //         let bound = BoundPosition::with_copies(Bound(decimal(30.75), decimal(175.35)), 6);

    //         let target = vec![
    //             BoundPosition {
    //                 buying: Bound(decimal(30.75), decimal(42.80)),
    //                 selling: Bound(decimal(66.90), decimal(78.95)),
    //                 position: Mutex::new(Position::None),
    //             },
    //             BoundPosition {
    //                 buying: Bound(decimal(54.85), decimal(66.90)),
    //                 selling: Bound(decimal(91.00), decimal(103.05)),
    //                 position: Mutex::new(Position::None),
    //             },
    //             BoundPosition {
    //                 buying: Bound(decimal(78.95), decimal(91.00)),
    //                 selling: Bound(decimal(115.10), decimal(127.15)),
    //                 position: Mutex::new(Position::None),
    //             },
    //             BoundPosition {
    //                 buying: Bound(decimal(103.05), decimal(115.10)),
    //                 selling: Bound(decimal(139.20), decimal(151.25)),
    //                 position: Mutex::new(Position::None),
    //             },
    //             BoundPosition {
    //                 buying: Bound(decimal(127.15), decimal(139.20)),
    //                 selling: Bound(decimal(163.30), decimal(175.35)),
    //                 position: Mutex::new(Position::None),
    //             },
    //         ];

    //         assert_eq!(bound, target);
    //     }

    // #[test]
    // fn test_predictive_lowest_profit_price() {
    //     let gride = Grid::new(decimal(50.0), (decimal(30.75), decimal(175.35)), 6, None);

    //     let target = vec![
    //         decimal(42.795720),
    //         decimal(66.906690),
    //         decimal(66.893310),
    //         decimal(91.009100),
    //         decimal(90.990900),
    //         decimal(115.11151),
    //         decimal(115.08849),
    //         decimal(139.21392),
    //         decimal(139.18608),
    //         decimal(163.31633),
    //     ];

    //     assert_eq!(gride.predictive_lowest_profit_price(), target);
    // }

    //     #[test]
    //     fn test_predictive_highest_profit_price() {
    //         let gride = Grid::new(decimal(50.0), (decimal(30.75), decimal(175.35)), 6, None);

    //         let target = vec![
    //             decimal(30.75307500),
    //             decimal(78.94210500),
    //             decimal(54.85548500),
    //             decimal(103.0396950),
    //             decimal(78.95789500),
    //             decimal(127.1372850),
    //             decimal(103.0603050),
    //             decimal(151.2348750),
    //             decimal(127.1627150),
    //             decimal(175.3324650),
    //         ];

    //         assert_eq!(gride.predictive_highest_profit_price(), target);
    //     }

    //     #[tokio::test]
    //     async fn test_position() {
    //         let positions = BoundPosition::with_copies(Bound(decimal(30.75), decimal(175.35)), 6);
    //         let target = Position::Stock(Order {
    //             price: decimal(50.0),
    //             amount: decimal(100.0),
    //             quantity: decimal(2.0),
    //             timestamp: 0,
    //         });
    //         {
    //             let mut lock = positions[0].position().lock().unwrap();
    //             *lock = target.clone();
    //         }

    //         assert_eq!(*(positions[0].position().lock().unwrap()), target);
    //     }

    //     #[tokio::test]
    //     #[traced_test]
    //     async fn test_grid_stop_loss() {
    //         let options = GridOptions {
    //             stop_loss: Some(decimal(0.05)),
    //         };
    //         let mut grid = Grid::new(
    //             decimal(100.0),
    //             (decimal(50.0), decimal(100.0)),
    //             4,
    //             Some(options),
    //         );

    //         assert_eq!(grid.is_none_position(), true);
    //         assert_eq!(grid.is_reach_stop_loss(&price(49.5)), false);
    //         assert_eq!(grid.is_reach_stop_loss(&price(48.0)), false);
    //         assert_eq!(grid.is_reach_stop_loss(&price(47.5)), true);
    //         assert_eq!(grid.is_reach_stop_loss(&price(45.0)), true);

    //         let order = Order {
    //             price: decimal(50.0),
    //             amount: decimal(50.0),
    //             quantity: decimal(1.0),
    //             timestamp: 0,
    //         };

    //         grid.positions
    //             .iter_mut()
    //             .for_each(|e| *e.position.lock().unwrap() = Position::Stock(order.clone()));

    //         assert_eq!(grid.is_none_position(), false);
    //         assert_eq!(grid.predictive_selling(&price(50.0)).await, None);
    //         assert_eq!(
    //             grid.predictive_selling(&price(47.5)).await,
    //             Some(vec![order.clone(); 3])
    //         );
    //         assert_eq!(
    //             grid.predictive_selling(&price(45.0)).await,
    //             Some(vec![order.clone(); 3])
    //         );
    //     }
}

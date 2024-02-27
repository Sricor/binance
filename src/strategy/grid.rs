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
        let investment = investment / Decimal::from(copies - 1);
        let interval = (range.high() - range.low()) / Decimal::from(copies);

        let investment = investment.trunc_with_scale(6);
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
        B: Fn(Price, Amount) -> ClosureFuture<QuantityPoint>,
        S: Fn(Price, Quantity) -> ClosureFuture<AmountPoint>,
    {
        self.limit.trap(price, buy, sell).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests_grid {
    use std::sync::Mutex;

    use super::super::tests_general::*;
    use super::*;

    impl PartialEq for LimitPosition {
        fn eq(&self, other: &Self) -> bool {
            self.investment == other.investment
                && self.buying == other.buying
                && self.selling == other.selling
                && *self.position.lock().unwrap() == *other.position.lock().unwrap()
        }
    }

    #[test]
    fn test_split_limit_position() {
        let positions = Grid::split(decimal(100.0), Range(decimal(50.0), decimal(90.0)), 4);
        let target = vec![
            LimitPosition {
                investment: decimal(33.333333),
                buying: Range(decimal(50.0), decimal(55.0)),
                selling: Range(decimal(65.0), decimal(70.0)),
                position: Mutex::new(None),
            },
            LimitPosition {
                investment: decimal(33.333333),
                buying: Range(decimal(60.0), decimal(65.0)),
                selling: Range(decimal(75.0), decimal(80.0)),
                position: Mutex::new(None),
            },
            LimitPosition {
                investment: decimal(33.333333),
                buying: Range(decimal(70.0), decimal(75.0)),
                selling: Range(decimal(85.0), decimal(90.0)),
                position: Mutex::new(None),
            },
        ];
        assert_eq!(positions, target);

        let positions = Grid::split(decimal(100.0), Range(decimal(50.0), decimal(90.0)), 3);
        let target = vec![
            LimitPosition {
                investment: decimal(50.0),
                buying: Range(decimal(50.0), decimal(56.66666650)),
                selling: Range(decimal(69.99999950), decimal(76.666666)),
                position: Mutex::new(None),
            },
            LimitPosition {
                investment: decimal(50.0),
                buying: Range(decimal(63.333333), decimal(69.99999950)),
                selling: Range(decimal(83.33333250), decimal(89.999999)),
                position: Mutex::new(None),
            },
        ];
        assert_eq!(positions, target);
    }

    #[test]
    fn test_predictive_lowest_profit_price() {
        let grid = Grid::new(
            decimal(50.0),
            Range(decimal(30.75), decimal(175.35)),
            6,
            None,
        );

        let target = vec![
            decimal(42.795720),
            decimal(66.906690),
            decimal(66.893310),
            decimal(91.009100),
            decimal(90.990900),
            decimal(115.11151),
            decimal(115.08849),
            decimal(139.21392),
            decimal(139.18608),
            decimal(163.31633),
        ];

        assert_eq!(grid.predictive_lowest_profit_price(), target);
    }

    #[test]
    fn test_predictive_highest_profit_price() {
        let grid = Grid::new(
            decimal(50.0),
            Range(decimal(30.75), decimal(175.35)),
            6,
            None,
        );

        let target = vec![
            decimal(30.75307500),
            decimal(78.94210500),
            decimal(54.85548500),
            decimal(103.0396950),
            decimal(78.95789500),
            decimal(127.1372850),
            decimal(103.0603050),
            decimal(151.2348750),
            decimal(127.1627150),
            decimal(175.3324650),
        ];

        assert_eq!(grid.predictive_highest_profit_price(), target);
    }
}

use std::sync::Mutex;

use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Serialize};
use tracing::{instrument, trace};

use super::{Order, Position, PositionSide, Strategy};
use crate::noun::*;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Bound(pub Decimal, pub Decimal);

impl Bound {
    pub fn is_within(&self, value: &Decimal) -> bool {
        if value > &self.0 && value < &self.1 {
            return true;
        }

        false
    }

    pub fn high(&self) -> &Decimal {
        if self.0 > self.1 {
            return &self.0;
        }

        &self.1
    }

    pub fn low(&self) -> &Decimal {
        if self.0 < self.1 {
            return &self.0;
        }

        &self.1
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoundPosition {
    buying: Bound,
    selling: Bound,
    position: Mutex<Position>,
}

impl BoundPosition {
    pub fn new(buying: Bound, selling: Bound, position: Option<Position>) -> Self {
        Self {
            buying,
            selling,
            position: Mutex::new(position.unwrap_or(Position::None)),
        }
    }

    pub fn with_copies(bound: Bound, copies: usize) -> Vec<Self> {
        let mut result = Vec::with_capacity(copies);
        let interval = (bound.high() - bound.low()) / Decimal::from(copies);
        let interval = interval.trunc_with_scale(6);

        for i in 0..copies - 1 {
            let buying = bound.low() + interval * Decimal::from(i);
            let selling = bound.low() + interval * Decimal::from(i + 2);
            result.push(Self::new(
                Bound(buying, buying + (interval / Decimal::TWO)),
                Bound(selling - (interval / Decimal::TWO), selling),
                None,
            ))
        }

        result
    }

    pub fn buying(&self) -> &Bound {
        &self.buying
    }

    pub fn selling(&self) -> &Bound {
        &self.selling
    }

    pub fn position(&self) -> &Mutex<Position> {
        &self.position
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Grid {
    bound: Bound,
    investment: Amount,
    positions: Vec<BoundPosition>,
    options: GridOptions,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GridOptions {
    pub stop_loss: Option<Decimal>,
}

impl Grid {
    pub fn new(
        investment: Amount,
        bound: (Price, Price),
        copies: usize,
        options: Option<GridOptions>,
    ) -> Self {
        let bound = Bound(bound.0, bound.1);
        let positions = BoundPosition::with_copies(bound.clone(), copies);
        let options = options.unwrap_or_default();

        Self {
            bound,
            investment,
            positions,
            options,
        }
    }

    pub fn predictive_lowest_profit_price(&self) -> Vec<Price> {
        let mut result = Vec::with_capacity(self.positions.len() + 1);
        for i in self.positions.iter() {
            let buying_price = i.buying.1 * Decimal::from_f64(0.9999).unwrap();
            let selling_price = i.selling.0 * Decimal::from_f64(1.0001).unwrap();
            result.push(buying_price.trunc_with_scale(8));
            result.push(selling_price.trunc_with_scale(8));
        }

        result
    }

    pub fn predictive_highest_profit_price(&self) -> Vec<Price> {
        let mut result = Vec::with_capacity(self.positions.len() + 1);
        for i in self.positions.iter() {
            let buying_price = i.buying.0 * Decimal::from_f64(1.0001).unwrap();
            let selling_price = i.selling.1 * Decimal::from_f64(0.9999).unwrap();
            result.push(buying_price.trunc_with_scale(8));
            result.push(selling_price.trunc_with_scale(8));
        }

        result
    }

    pub fn find_buying_bound_position(&self, price: &Price) -> Option<&BoundPosition> {
        let index = self
            .positions
            .iter()
            .position(|e| e.buying.is_within(price))?;

        let bound = &self.positions[index];
        trace!("found the buying bound {:?}", bound);

        Some(bound)
    }

    pub fn find_selling_bound_position(&self, price: &Price) -> Option<&BoundPosition> {
        let index = self
            .positions
            .iter()
            .position(|e| e.selling.is_within(price))?;

        let bound = &self.positions[index];
        trace!("found the selling bound {:?}", bound);

        Some(bound)
    }

    pub fn positions(&self) -> &Vec<BoundPosition> {
        &self.positions
    }

    pub fn is_reach_stop_loss(&self, price: &Price) -> bool {
        if let Some(stop) = &self.options.stop_loss {
            if price <= &(self.bound.low() * (Decimal::ONE - stop)) {
                return true;
            }
        }

        false
    }
}

impl Strategy for Grid {
    #[instrument(skip(self))]
    async fn predictive_buying(&self, price: &Price) -> Option<Amount> {
        let bound = self.find_buying_bound_position(price)?;
        let position = bound.position.lock().unwrap();

        if let Position::None = &*position {
            let target = self.investment / Decimal::from(self.positions.len() + 1);
            trace!("predictive buy {} amount", target);
            return Some(target);
        }

        None
    }

    #[instrument(skip(self))]
    async fn predictive_selling(&self, price: &Price) -> Option<Vec<Order>> {
        if self.is_reach_stop_loss(price) {
            trace!("reach the stop loss price {:?}", price);
            let mut result = Vec::with_capacity(self.positions.len() + 1);
            self.positions.iter().for_each(|e| {
                let position = e.position.lock().unwrap();
                if let Position::Stock(order) = &*position {
                    result.push(order.clone())
                }
            });
            trace!("stop loss, predictive sell order {:?}", result);
            return Some(result);
        }

        let bound = self.find_selling_bound_position(price)?;
        let position = bound.position.lock().unwrap();

        if let Position::Stock(order) = &*position {
            trace!("predictive sell order {:?}", order);
            return Some(vec![order.clone()]);
        }

        None
    }

    #[instrument(skip(self))]
    async fn update_position(&self, side: &PositionSide) -> () {
        match side {
            PositionSide::Increase(v) => {
                let bound = self.find_buying_bound_position(&v.price).unwrap();

                // TODO: is stock?
                let mut position = bound.position.lock().unwrap();
                *position = Position::Stock(v.clone());
            }
            PositionSide::Decrease(v) => {
                let bound = self.find_buying_bound_position(&v.price).unwrap();

                // TODO: is none?
                let mut position = bound.position.lock().unwrap();
                *position = Position::None;
            }
        };
    }

    fn is_completed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    impl PartialEq for BoundPosition {
        fn eq(&self, other: &Self) -> bool {
            self.buying == other.buying && self.selling == other.selling
        }
    }

    use rust_decimal::prelude::FromPrimitive;
    use tracing_test::traced_test;

    use super::*;

    fn decimal(value: f64) -> Decimal {
        Decimal::from_f64(value).unwrap()
    }

    #[test]
    fn test_bound_position_with_copies_one() {
        let bound = BoundPosition::with_copies(Bound(decimal(50.0), decimal(90.0)), 4);
        let target = vec![
            BoundPosition {
                buying: Bound(decimal(50.0), decimal(55.0)),
                selling: Bound(decimal(65.0), decimal(70.0)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(decimal(60.0), decimal(65.0)),
                selling: Bound(decimal(75.0), decimal(80.0)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(decimal(70.0), decimal(75.0)),
                selling: Bound(decimal(85.0), decimal(90.0)),
                position: Mutex::new(Position::None),
            },
        ];
        assert_eq!(bound, target);

        let bound = BoundPosition::with_copies(Bound(decimal(50.0), decimal(90.0)), 3);
        let target = vec![
            BoundPosition {
                buying: Bound(decimal(50.0), decimal(56.66666650)),
                selling: Bound(decimal(69.99999950), decimal(76.666666)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(decimal(63.333333), decimal(69.99999950)),
                selling: Bound(decimal(83.33333250), decimal(89.999999)),
                position: Mutex::new(Position::None),
            },
        ];
        assert_eq!(bound, target);
    }

    #[test]
    fn test_bound_position_with_copies_two() {
        let bound = BoundPosition::with_copies(Bound(decimal(30.75), decimal(175.35)), 6);

        let target = vec![
            BoundPosition {
                buying: Bound(decimal(30.75), decimal(42.80)),
                selling: Bound(decimal(66.90), decimal(78.95)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(decimal(54.85), decimal(66.90)),
                selling: Bound(decimal(91.00), decimal(103.05)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(decimal(78.95), decimal(91.00)),
                selling: Bound(decimal(115.10), decimal(127.15)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(decimal(103.05), decimal(115.10)),
                selling: Bound(decimal(139.20), decimal(151.25)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(decimal(127.15), decimal(139.20)),
                selling: Bound(decimal(163.30), decimal(175.35)),
                position: Mutex::new(Position::None),
            },
        ];

        assert_eq!(bound, target);
    }

    #[test]
    fn test_predictive_lowest_profit_price() {
        let gride = Grid::new(decimal(50.0), (decimal(30.75), decimal(175.35)), 6, None);

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

        assert_eq!(gride.predictive_lowest_profit_price(), target);
    }

    #[test]
    fn test_predictive_highest_profit_price() {
        let gride = Grid::new(decimal(50.0), (decimal(30.75), decimal(175.35)), 6, None);

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

        assert_eq!(gride.predictive_highest_profit_price(), target);
    }

    #[tokio::test]
    async fn test_position() {
        let positions = BoundPosition::with_copies(Bound(decimal(30.75), decimal(175.35)), 6);
        let target = Position::Stock(Order {
            price: decimal(50.0),
            amount: decimal(100.0),
            quantity: decimal(2.0),
            timestamp: 0,
        });
        {
            let mut lock = positions[0].position().lock().unwrap();
            *lock = target.clone();
        }

        assert_eq!(*(positions[0].position().lock().unwrap()), target);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_grid_stop_loss() {
        let options = GridOptions {
            stop_loss: Some(decimal(0.05)),
        };
        let mut grid = Grid::new(
            decimal(100.0),
            (decimal(50.0), decimal(100.0)),
            4,
            Some(options),
        );

        assert_eq!(grid.is_reach_stop_loss(&decimal(49.5)), false);
        assert_eq!(grid.is_reach_stop_loss(&decimal(48.0)), false);
        assert_eq!(grid.is_reach_stop_loss(&decimal(47.5)), true);
        assert_eq!(grid.is_reach_stop_loss(&decimal(45.0)), true);

        let order = Order {
            price: decimal(50.0),
            amount: decimal(50.0),
            quantity: decimal(1.0),
            timestamp: 0,
        };

        grid.positions
            .iter_mut()
            .for_each(|e| *e.position.lock().unwrap() = Position::Stock(order.clone()));

        assert_eq!(grid.predictive_selling(&decimal(50.0)).await, None);
        assert_eq!(
            grid.predictive_selling(&decimal(47.5)).await,
            Some(vec![order.clone(); 3])
        );
        assert_eq!(
            grid.predictive_selling(&decimal(45.0)).await,
            Some(vec![order.clone(); 3])
        );
    }
}

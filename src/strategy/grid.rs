use parking_lot::Mutex;
use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize)]
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

pub struct Grid {
    investment: Amount,
    positions: Vec<BoundPosition>,
}

impl Grid {
    pub fn new(investment: Amount, positions: Vec<BoundPosition>) -> Self {
        Self {
            investment,
            positions,
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

    pub fn find_bound_position(&self, price: &Price) -> Option<&BoundPosition> {
        let index = self
            .positions
            .iter()
            .position(|e| e.buying.is_within(price) || e.selling.is_within(price))?;

        Some(&self.positions[index])
    }

    pub fn positions(&self) -> &Vec<BoundPosition> {
        &self.positions
    }
}

impl Strategy for Grid {
    async fn predictive_buy(&self, price: &Price) -> Option<Amount> {
        let bound = self.find_bound_position(price)?;
        let position = bound.position.lock();

        if let Position::None = &*position {
            return Some(self.investment / Decimal::from(self.positions.len() + 1));
        }

        None
    }

    async fn predictive_sell(&self, price: &Price) -> Option<Vec<Order>> {
        let bound = self.find_bound_position(price)?;
        let position = bound.position.lock();

        if let Position::Stock(v) = &*position {
            return Some(vec![v.clone()]);
        }

        None
    }

    async fn update_position(&self, side: &PositionSide) -> () {
        match side {
            PositionSide::Increase(v) => {
                let bound = self.find_bound_position(&v.price).unwrap();

                // TODO: is stock?
                let mut position = bound.position.lock();
                *position = Position::Stock(v.clone());
            }
            PositionSide::Decrease(v) => {
                let bound = self.find_bound_position(&v.price).unwrap();

                // TODO: is none?
                let mut position = bound.position.lock();
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

    impl std::fmt::Debug for BoundPosition {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("BoundPosition")
                .field("buying", &self.buying)
                .field("selling", &self.selling)
                .finish()
        }
    }

    use rust_decimal::prelude::FromPrimitive;

    use super::*;

    fn to_decimal(value: f64) -> Decimal {
        Decimal::from_f64(value).unwrap()
    }

    #[test]
    fn test_bound_position_with_copies_one() {
        let bound = BoundPosition::with_copies(Bound(to_decimal(50.0), to_decimal(90.0)), 4);
        let target = vec![
            BoundPosition {
                buying: Bound(to_decimal(50.0), to_decimal(55.0)),
                selling: Bound(to_decimal(65.0), to_decimal(70.0)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(to_decimal(60.0), to_decimal(65.0)),
                selling: Bound(to_decimal(75.0), to_decimal(80.0)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(to_decimal(70.0), to_decimal(75.0)),
                selling: Bound(to_decimal(85.0), to_decimal(90.0)),
                position: Mutex::new(Position::None),
            },
        ];
        assert_eq!(bound, target);

        let bound = BoundPosition::with_copies(Bound(to_decimal(50.0), to_decimal(90.0)), 3);
        let target = vec![
            BoundPosition {
                buying: Bound(to_decimal(50.0), to_decimal(56.66666650)),
                selling: Bound(to_decimal(69.99999950), to_decimal(76.666666)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(to_decimal(63.333333), to_decimal(69.99999950)),
                selling: Bound(to_decimal(83.33333250), to_decimal(89.999999)),
                position: Mutex::new(Position::None),
            },
        ];
        assert_eq!(bound, target);
    }

    #[test]
    fn test_bound_position_with_copies_two() {
        let bound = BoundPosition::with_copies(Bound(to_decimal(30.75), to_decimal(175.35)), 6);

        let target = vec![
            BoundPosition {
                buying: Bound(to_decimal(30.75), to_decimal(42.80)),
                selling: Bound(to_decimal(66.90), to_decimal(78.95)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(to_decimal(54.85), to_decimal(66.90)),
                selling: Bound(to_decimal(91.00), to_decimal(103.05)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(to_decimal(78.95), to_decimal(91.00)),
                selling: Bound(to_decimal(115.10), to_decimal(127.15)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(to_decimal(103.05), to_decimal(115.10)),
                selling: Bound(to_decimal(139.20), to_decimal(151.25)),
                position: Mutex::new(Position::None),
            },
            BoundPosition {
                buying: Bound(to_decimal(127.15), to_decimal(139.20)),
                selling: Bound(to_decimal(163.30), to_decimal(175.35)),
                position: Mutex::new(Position::None),
            },
        ];

        assert_eq!(bound, target);
    }

    #[test]
    fn test_predictive_lowest_profit_price() {
        let positions = BoundPosition::with_copies(Bound(to_decimal(30.75), to_decimal(175.35)), 6);
        let gride = Grid::new(to_decimal(50.0), positions);

        let target = vec![
            to_decimal(42.795720),
            to_decimal(66.906690),
            to_decimal(66.893310),
            to_decimal(91.009100),
            to_decimal(90.990900),
            to_decimal(115.11151),
            to_decimal(115.08849),
            to_decimal(139.21392),
            to_decimal(139.18608),
            to_decimal(163.31633),
        ];

        assert_eq!(gride.predictive_lowest_profit_price(), target);
    }

    #[test]
    fn test_predictive_highest_profit_price() {
        let positions = BoundPosition::with_copies(Bound(to_decimal(30.75), to_decimal(175.35)), 6);
        let gride = Grid::new(to_decimal(50.0), positions);

        let target = vec![
            to_decimal(30.75307500),
            to_decimal(78.94210500),
            to_decimal(54.85548500),
            to_decimal(103.0396950),
            to_decimal(78.95789500),
            to_decimal(127.1372850),
            to_decimal(103.0603050),
            to_decimal(151.2348750),
            to_decimal(127.1627150),
            to_decimal(175.3324650),
        ];

        assert_eq!(gride.predictive_highest_profit_price(), target);
    }

    #[tokio::test]
    async fn test_position() {
        let positions = BoundPosition::with_copies(Bound(to_decimal(30.75), to_decimal(175.35)), 6);
        let target = Position::Stock(Order {
            price: to_decimal(50.0),
            amount: to_decimal(100.0),
            quantity: to_decimal(2.0),
            timestamp: 0,
        });
        {
            let mut lock = positions[0].position().lock();
            *lock = target.clone();
        }

        assert_eq!(*(positions[0].position().lock()), target);
    }
}

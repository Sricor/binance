use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

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

    fn find_bound(&self, price: &Price) -> Option<usize> {
        self.positions
            .iter()
            .position(|e| e.buying.is_within(price) || e.selling.is_within(price))
    }
}

impl Strategy for Grid {
    async fn predictive_buy(&self, price: &Price) -> Option<Amount> {
        let index = self.find_bound(price)?;
        let bound = &self.positions[index];
        let position = bound.position.lock().await;

        if let Position::None = &*position {
            return Some(self.investment / Decimal::from(self.positions.len() + 1));
        }

        None
    }

    async fn predictive_sell(&self, price: &Price) -> Option<Vec<Order>> {
        let index = self.find_bound(price)?;
        let bound = &self.positions[index];
        let position = bound.position.lock().await;

        if let Position::Stock(v) = &*position {
            return Some(vec![v.clone()]);
        }

        None
    }

    async fn update_position(&self, side: &PositionSide) -> () {
        match side {
            PositionSide::Increase(v) => {
                let index = self.find_bound(&v.price).unwrap();
                let bound = &self.positions[index];

                // TODO: is stock?
                let mut position = bound.position.lock().await;
                *position = Position::Stock(v.clone());
            }
            PositionSide::Decrease(v) => {
                let index = self.find_bound(&v.price).unwrap();
                let bound = &self.positions[index];

                // TODO: is none?
                let mut position = bound.position.lock().await;
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
}

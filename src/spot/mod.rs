use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::noun::*;

// pub mod client;
pub mod error;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Spot {
    /// Trading currency pairs
    pub symbol: Symbol,

    /// Transaction precision
    pub transaction_quantity_precision: Precision,

    /// Holding quantity precision
    pub quantity_precision: Precision,

    /// Income amount precision
    pub amount_precision: Precision,

    /// Buying commission
    pub buying_commission: Commission,

    /// Selling commission
    pub selling_commission: Commission,

    /// Minimum transaction amount
    pub minimum_transaction_amount: Amount,
}

impl Spot {
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    // Calculating the buying commission fee, the actual holding quantity
    pub fn buying_quantity_with_commission(&self, quantity: &Quantity) -> Quantity {
        (quantity * (Decimal::ONE - self.buying_commission)).round_dp(self.quantity_precision)
    }

    // Accurate the quantity to meet the transaction accuracy requirements
    pub fn transaction_quantity_with_precision(&self, quantity: &Quantity) -> Quantity {
        quantity.trunc_with_scale(self.transaction_quantity_precision)
    }

    // Calculate earnings after upfront selling commission fees
    pub fn selling_amount_with_commission(&self, amount: &Amount) -> Amount {
        let commission = (amount * self.selling_commission).round_dp(self.amount_precision);
        amount - commission
    }

    pub fn selling_income_amount(&self, price: &Price, quantity: &Quantity) -> Amount {
        price * quantity
    }

    pub fn buying_spent_amount(&self, price: &Price, quantity: &Quantity) -> Amount {
        price * quantity
    }

    pub fn is_reached_minimum_transaction_limit(&self, price: &Price, quantity: &Quantity) -> bool {
        if price * quantity > self.minimum_transaction_amount {
            return true;
        }

        false
    }

    pub fn buying_quantity_by_amount(&self, price: &Price, amount: &Amount) -> Quantity {
        self.transaction_quantity_with_precision(&(amount / price))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotBuying {
    /// Buying price
    pub price: Price,

    /// Buying quantity
    pub quantity: Quantity,

    /// Amount spent on buying
    pub spent: Amount,

    /// Buying quantity after commission, also the actual quantity held
    pub quantity_after_commission: Quantity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotSelling {
    /// Selling price
    pub price: Price,

    /// Selling quantity
    pub quantity: Quantity,

    /// Income gained after selling
    pub income: Amount,

    /// Income gained after commission selling, also the actual income recorded
    pub income_after_commission: Amount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotTransaction {
    buying: SpotBuying,
    selling: SpotSelling,

    /// Net profit does not include leave quantity
    net_profit: Amount,

    /// Return on investment (ROI)
    net_profit_margin: Decimal,

    /// Quantity after transaction
    leave_quantity: Quantity,
}

impl SpotTransaction {
    pub fn new(buying: SpotBuying, selling: SpotSelling) -> Self {
        let net_profit = selling.income_after_commission - buying.spent;
        let net_profit_margin = (net_profit / buying.spent).round_dp(6);
        let leave_quantity = buying.quantity_after_commission - selling.quantity;

        Self {
            buying,
            selling,
            net_profit,
            net_profit_margin,
            leave_quantity,
        }
    }

    pub fn net_profit(&self) -> &Amount {
        &self.net_profit
    }

    pub fn net_profit_margin(&self) -> &Decimal {
        &self.net_profit_margin
    }

    pub fn leave_quantity(&self) -> &Quantity {
        &self.leave_quantity
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::prelude::FromPrimitive;

    use super::*;

    fn btc_spot() -> Spot {
        Spot {
            symbol: "BTCUSDT".into(),
            transaction_quantity_precision: 5,
            quantity_precision: 7, // BTC Precision
            amount_precision: 8,   // USDT Precision
            minimum_transaction_amount: Decimal::from(5),
            buying_commission: Decimal::from_f64(0.001).unwrap(),
            selling_commission: Decimal::from_f64(0.001).unwrap(),
        }
    }

    fn eth_spot() -> Spot {
        Spot {
            symbol: "ETHUSDT".into(),
            transaction_quantity_precision: 4,
            quantity_precision: 7, // ETH Precision
            amount_precision: 8,   // USDT Precision
            minimum_transaction_amount: Decimal::from(5),
            buying_commission: Decimal::from_f64(0.001).unwrap(),
            selling_commission: Decimal::from_f64(0.001).unwrap(),
        }
    }

    fn buying_spot_one() -> SpotBuying {
        SpotBuying {
            price: Decimal::from_f64(100.23).unwrap(),
            quantity: Decimal::from_f64(1.49).unwrap(),
            spent: Decimal::from_f64(149.3427).unwrap(),
            quantity_after_commission: Decimal::from_f64(1.48851).unwrap(),
        }
    }

    fn selling_spot_one() -> SpotSelling {
        SpotSelling {
            price: Decimal::from_f64(112.58).unwrap(),
            quantity: Decimal::from_f64(1.48).unwrap(),
            income: Decimal::from_f64(166.6184).unwrap(),
            income_after_commission: Decimal::from_f64(166.4517816).unwrap(),
        }
    }

    fn buying_spot_two() -> SpotBuying {
        SpotBuying {
            price: Decimal::from_f64(200.58).unwrap(),
            quantity: Decimal::from_f64(2.59248).unwrap(),
            spent: Decimal::from_f64(519.9996384).unwrap(),
            quantity_after_commission: Decimal::from_f64(2.5898875).unwrap(),
        }
    }

    fn selling_spot_two() -> SpotSelling {
        SpotSelling {
            price: Decimal::from_f64(112.69).unwrap(),
            quantity: Decimal::from_f64(2.58988).unwrap(),
            income: Decimal::from_f64(291.8535772).unwrap(),
            income_after_commission: Decimal::from_f64(291.56172362).unwrap(),
        }
    }

    #[test]
    fn test_buying_quantity_with_commission() {
        let quantity =
            btc_spot().buying_quantity_with_commission(&Decimal::from_f64(0.00985).unwrap());
        assert_eq!(quantity, Decimal::from_f64(0.0098402).unwrap());

        let quantity =
            btc_spot().buying_quantity_with_commission(&Decimal::from_f64(0.0008).unwrap());
        assert_eq!(quantity, Decimal::from_f64(0.0007992).unwrap());

        let quantity =
            eth_spot().buying_quantity_with_commission(&Decimal::from_f64(0.0025).unwrap());
        assert_eq!(quantity, Decimal::from_f64(0.0024975).unwrap());
    }

    #[test]
    fn test_transaction_quantity_with_precision() {
        let quantity =
            btc_spot().transaction_quantity_with_precision(&Decimal::from_f64(0.00985231).unwrap());
        assert_eq!(quantity, Decimal::from_f64(0.00985).unwrap());

        let quantity =
            btc_spot().transaction_quantity_with_precision(&Decimal::from_f64(0.0008561).unwrap());
        assert_eq!(quantity, Decimal::from_f64(0.00085).unwrap());

        let quantity =
            eth_spot().transaction_quantity_with_precision(&Decimal::from_f64(0.002372).unwrap());
        assert_eq!(quantity, Decimal::from_f64(0.0023).unwrap());
    }

    #[test]
    fn test_selling_amount_with_commission() {
        let amount =
            btc_spot().selling_amount_with_commission(&Decimal::from_f64(65.8308373).unwrap());
        assert_eq!(amount, Decimal::from_f64(65.76500646).unwrap());

        let amount =
            btc_spot().selling_amount_with_commission(&Decimal::from_f64(16.4650161).unwrap());
        assert_eq!(amount, Decimal::from_f64(16.44855108).unwrap());

        let amount =
            eth_spot().selling_amount_with_commission(&Decimal::from_f64(12.731936).unwrap());
        assert_eq!(amount, Decimal::from_f64(12.71920406).unwrap());
    }

    #[test]
    fn test_is_allow_transaction() {
        let allow = btc_spot().is_reached_minimum_transaction_limit(
            &Decimal::from_f64(10.0).unwrap(),
            &Decimal::from_f64(0.0025).unwrap(),
        );
        assert_eq!(allow, false);

        let allow = btc_spot().is_reached_minimum_transaction_limit(
            &Decimal::from_f64(5.0).unwrap(),
            &Decimal::from_f64(2.0).unwrap(),
        );
        assert_eq!(allow, true);

        let allow = btc_spot().is_reached_minimum_transaction_limit(
            &Decimal::from_f64(30.5).unwrap(),
            &Decimal::from_f64(2.0).unwrap(),
        );

        assert_eq!(allow, true);
        let allow = btc_spot().is_reached_minimum_transaction_limit(
            &Decimal::from_f64(100.5).unwrap(),
            &Decimal::from_f64(0.00025).unwrap(),
        );
        assert_eq!(allow, false);
    }

    #[test]
    fn test_buying_quantity_by_amount() {
        let quantity = btc_spot().buying_quantity_by_amount(
            &Decimal::from_f64(68.25).unwrap(),
            &Decimal::from_f64(215.32).unwrap(),
        );
        assert_eq!(quantity, Decimal::from_f64(3.15487).unwrap());

        let quantity = eth_spot().buying_quantity_by_amount(
            &Decimal::from_f64(9854.12).unwrap(),
            &Decimal::from_f64(300.5961).unwrap(),
        );
        assert_eq!(quantity, Decimal::from_f64(0.03050).unwrap());
    }

    #[test]
    fn test_transaction_net_profit() {
        let transaction = SpotTransaction::new(buying_spot_one(), selling_spot_one());
        assert_eq!(
            transaction.net_profit().clone(),
            Decimal::from_f64(17.1090816).unwrap()
        );

        let transaction = SpotTransaction::new(buying_spot_two(), selling_spot_two());
        assert_eq!(
            transaction.net_profit().clone(),
            Decimal::from_f64(-228.43791478).unwrap()
        );
    }

    #[test]
    fn test_transaction_net_profit_margin() {
        let transaction = SpotTransaction::new(buying_spot_one(), selling_spot_one());
        assert_eq!(
            transaction.net_profit_margin().clone(),
            Decimal::from_f64(0.114563).unwrap()
        );

        let transaction = SpotTransaction::new(buying_spot_two(), selling_spot_two());
        assert_eq!(
            transaction.net_profit_margin().clone(),
            Decimal::from_f64(-0.439304).unwrap()
        );
    }

    #[test]
    fn test_transaction_leave_quantity() {
        let transaction = SpotTransaction::new(buying_spot_one(), selling_spot_one());
        assert_eq!(
            transaction.leave_quantity().clone(),
            Decimal::from_f64(0.00851).unwrap()
        );

        let transaction = SpotTransaction::new(buying_spot_two(), selling_spot_two());
        assert_eq!(
            transaction.leave_quantity().clone(),
            Decimal::from_f64(0.0000075).unwrap()
        );
    }
}

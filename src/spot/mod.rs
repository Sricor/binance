use rust_decimal::Decimal;

pub mod client;
pub mod error;

use crate::noun::*;

pub struct Spot {
    symbol: Symbol,
    transaction_quantity_precision: Precision,

    holding_quantity_precision: Precision,
    amount_income_precision: Precision,
    buying_commission: Commission,
    selling_commission: Commission,
    minimum_transaction_amount: Amount,
}

impl Spot {
    pub fn new(
        symbol: Symbol,
        transaction_quantity_precision: Precision,
        holding_quantity_precision: Precision,
        amount_income_precision: Precision,
        buying_commission: Commission,
        selling_commission: Commission,
        minimum_transaction_amount: Amount,
    ) -> Self {
        Self {
            symbol,
            transaction_quantity_precision,
            holding_quantity_precision,
            amount_income_precision,
            buying_commission,
            selling_commission,
            minimum_transaction_amount,
        }
    }

    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    // Calculating the buying commission fee, the actual holding quantity
    pub fn buying_quantity_with_commission(&self, quantity: &Quantity) -> Quantity {
        (quantity * (Decimal::ONE - self.buying_commission))
            .round_dp(self.holding_quantity_precision)
    }

    // Accurate the quantity to meet the transaction accuracy requirements
    pub fn transaction_quantity_with_precision(&self, quantity: &Quantity) -> Quantity {
        quantity.trunc_with_scale(self.transaction_quantity_precision)
    }

    // Calculate earnings after upfront selling commission fees
    pub fn selling_amount_with_commission(&self, amount: &Amount) -> Amount {
        let commission = (amount * self.selling_commission).round_dp(self.amount_income_precision);
        amount - commission
    }

    pub fn selling_income_amount(&self, price: &Price, quantity: &Quantity) -> Amount {
        price * quantity
    }

    pub fn buying_spent_amount(&self, price: &Price, quantity: &Quantity) -> Amount {
        price * quantity
    }

    pub fn is_allow_transaction(&self, price: &Price, quantity: &Quantity) -> bool {
        if price * quantity > self.minimum_transaction_amount {
            return true;
        }

        false
    }

    pub fn buying_quantity_by_amount(&self, price: &Price, amount: &Amount) -> Quantity {
        self.transaction_quantity_with_precision(&(amount / price))
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
            holding_quantity_precision: 7, // BTC Precision
            amount_income_precision: 8,    // USDT Precision
            minimum_transaction_amount: Decimal::from(5),
            buying_commission: Decimal::from_f64(0.001).unwrap(),
            selling_commission: Decimal::from_f64(0.001).unwrap(),
        }
    }

    fn eth_spot() -> Spot {
        Spot {
            symbol: "ETHUSDT".into(),
            transaction_quantity_precision: 4,
            holding_quantity_precision: 7, // ETH Precision
            amount_income_precision: 8,    // USDT Precision
            minimum_transaction_amount: Decimal::from(5),
            buying_commission: Decimal::from_f64(0.001).unwrap(),
            selling_commission: Decimal::from_f64(0.001).unwrap(),
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
        let allow = btc_spot().is_allow_transaction(
            &Decimal::from_f64(10.0).unwrap(),
            &Decimal::from_f64(0.0025).unwrap(),
        );
        assert_eq!(allow, false);

        let allow = btc_spot().is_allow_transaction(
            &Decimal::from_f64(5.0).unwrap(),
            &Decimal::from_f64(2.0).unwrap(),
        );
        assert_eq!(allow, true);

        let allow = btc_spot().is_allow_transaction(
            &Decimal::from_f64(30.5).unwrap(),
            &Decimal::from_f64(2.0).unwrap(),
        );

        assert_eq!(allow, true);
        let allow = btc_spot().is_allow_transaction(
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
}

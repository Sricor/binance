// use rust_binance;
// use rust_decimal::{prelude::FromPrimitive, Decimal};

// fn to_decimal(value: f64) -> Decimal {
//     Decimal::from_f64(value).unwrap()
// }

// fn to_decimals(value: Vec<f64>) -> Vec<Decimal> {
//     value.into_iter().map(|e| to_decimal(e)).collect()
// }

// fn price_up_ten_percent() -> Vec<Decimal> {
//     let v = vec![
//         100.00, 101.00, 102.00, 101.50, 103.00, 104.00, 103.50, 105.00, 106.00, 105.50, 107.00,
//         108.00, 107.50, 109.00, 110.00, 110.50, 109.50,
//     ];

//     to_decimals(v)
// }

// fn price_same() -> Vec<Decimal> {
//     let v = vec![100.00, 100.00, 100.00, 100.00, 100.00, 100.00, 100.00];

//     to_decimals(v)
// }

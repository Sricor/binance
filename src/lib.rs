mod common;

pub mod market;
pub mod spot;
pub mod strategy;
pub mod treasurer;

pub mod noun {
    pub use rust_decimal::Decimal;

    pub type Symbol = String;
    pub type Price = Decimal;
    pub type Precision = u32;
    pub type Quantity = Decimal;
    pub type Commission = Decimal;
    pub type Amount = Decimal;
}

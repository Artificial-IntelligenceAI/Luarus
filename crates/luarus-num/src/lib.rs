//! Exact arbitrary-precision arithmetic: the `er` type'''s insides.

pub mod rational;
pub mod uint;

pub use rational::Rational;
pub use uint::BigUint;

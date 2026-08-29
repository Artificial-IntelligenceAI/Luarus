//! Exact rational numbers: a sign, a numerator and a denominator, all
//! arbitrary-precision.
//!
//! This is what `er` is made of. Every value is kept in lowest terms with a
//! non-zero denominator, so equality is structural and `'1' / '3' * '3'` is
//! exactly `'1'` rather than nearly it. Nothing here overflows; the only
//! arithmetic failure is division by zero.

use std::cmp::Ordering;

use crate::uint::BigUint;

#[derive(Clone, Debug)]
pub struct Rational {
    /// Zero is never negative, so every value has one representation.
    negative: bool,
    num: BigUint,
    /// Never zero, and share no factor with `num`.
    den: BigUint,
}

impl PartialEq for Rational {
    fn eq(&self, other: &Self) -> bool {
        self.negative == other.negative && self.num == other.num && self.den == other.den
    }
}

impl Eq for Rational {}

impl Rational {
    pub fn zero() -> Self {
        Rational { negative: false, num: BigUint::zero(), den: BigUint::one() }
    }

    pub fn one() -> Self {
        Rational { negative: false, num: BigUint::one(), den: BigUint::one() }
    }

    /// Build from parts, reducing to lowest terms. `None` if `den` is zero.
    pub fn new(negative: bool, num: BigUint, den: BigUint) -> Option<Self> {
        if den.is_zero() {
            return None;
        }
        if num.is_zero() {
            return Some(Rational::zero());
        }
        let g = num.gcd(&den);
        let num = num.divmod(&g).expect("gcd is not zero").0;
        let den = den.divmod(&g).expect("gcd is not zero").0;
        Some(Rational { negative, num, den })
    }

    pub fn from_i64(v: i64) -> Self {
        Rational {
            negative: v < 0,
            num: BigUint::from_u64(v.unsigned_abs()),
            den: BigUint::one(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }

    pub fn numerator(&self) -> &BigUint {
        &self.num
    }

    pub fn denominator(&self) -> &BigUint {
        &self.den
    }

    pub fn is_integer(&self) -> bool {
        self.den.is_one()
    }

    pub fn neg(&self) -> Rational {
        if self.is_zero() {
            return self.clone();
        }
        Rational { negative: !self.negative, ..self.clone() }
    }

    pub fn abs(&self) -> Rational {
        Rational { negative: false, ..self.clone() }
    }

    pub fn add(&self, other: &Rational) -> Rational {
        // a/b + c/d over the common denominator b*d, with the sign worked out
        // from the two cross products rather than from the inputs.
        let left = self.num.mul(&other.den);
        let right = other.num.mul(&self.den);
        let den = self.den.mul(&other.den);

        if self.negative == other.negative {
            Rational::new(self.negative, left.add(&right), den).expect("denominators are non-zero")
        } else {
            match left.cmp_to(&right) {
                Ordering::Equal => Rational::zero(),
                Ordering::Greater => Rational::new(
                    self.negative,
                    left.sub(&right).expect("left is greater"),
                    den,
                )
                .expect("denominators are non-zero"),
                Ordering::Less => Rational::new(
                    other.negative,
                    right.sub(&left).expect("right is greater"),
                    den,
                )
                .expect("denominators are non-zero"),
            }
        }
    }

    pub fn sub(&self, other: &Rational) -> Rational {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &Rational) -> Rational {
        Rational::new(
            self.negative != other.negative,
            self.num.mul(&other.num),
            self.den.mul(&other.den),
        )
        .expect("denominators are non-zero")
    }

    /// `None` when `other` is zero.
    pub fn div(&self, other: &Rational) -> Option<Rational> {
        if other.is_zero() {
            return None;
        }
        Rational::new(
            self.negative != other.negative,
            self.num.mul(&other.den),
            self.den.mul(&other.num),
        )
    }

    /// The integer part, rounded toward zero.
    pub fn trunc(&self) -> Rational {
        let (q, _) = self.num.divmod(&self.den).expect("denominator is non-zero");
        Rational::new(self.negative, q, BigUint::one()).expect("one is non-zero")
    }

    /// Truncated remainder: `self - other * trunc(self / other)`, exactly as the
    /// integer types do it. `None` when `other` is zero.
    pub fn rem(&self, other: &Rational) -> Option<Rational> {
        let q = self.div(other)?.trunc();
        Some(self.sub(&other.mul(&q)))
    }

    pub fn cmp_to(&self, other: &Rational) -> Ordering {
        match (self.negative, other.negative) {
            (false, true) => return Ordering::Greater,
            (true, false) => return Ordering::Less,
            _ => {}
        }
        // Same sign: compare the cross products, and flip if both are negative.
        let left = self.num.mul(&other.den);
        let right = other.num.mul(&self.den);
        let ord = left.cmp_to(&right);
        if self.negative {
            ord.reverse()
        } else {
            ord
        }
    }

    /// Parse an `er` literal: an integer, a decimal, or a fraction.
    pub fn parse(text: &str) -> Option<Rational> {
        let cleaned: String = text.chars().filter(|c| *c != '_').collect();
        let body = cleaned.trim();
        let (negative, body) = match body.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, body.strip_prefix('+').unwrap_or(body)),
        };
        if body.is_empty() {
            return None;
        }

        if let Some((num, den)) = body.split_once('/') {
            // A fraction may not itself contain a decimal point.
            let num = BigUint::parse_decimal(num)?;
            let den = BigUint::parse_decimal(den)?;
            return Rational::new(negative, num, den);
        }

        match body.split_once('.') {
            None => Rational::new(negative, BigUint::parse_decimal(body)?, BigUint::one()),
            Some((whole, frac)) => {
                if frac.contains('.') {
                    return None;
                }
                // "1.25" is 125/100, then reduced.
                let whole = if whole.is_empty() {
                    BigUint::zero()
                } else {
                    BigUint::parse_decimal(whole)?
                };
                let frac_digits = if frac.is_empty() { BigUint::zero() } else { BigUint::parse_decimal(frac)? };
                let scale = BigUint::pow_small(10, frac.len() as u32);
                let num = whole.mul(&scale).add(&frac_digits);
                Rational::new(negative, num, scale)
            }
        }
    }
}

impl std::fmt::Display for Rational {
    /// Print exactly, and print back what could be read in again.
    ///
    /// A fraction whose denominator is only twos and fives has a terminating
    /// decimal expansion, and reads better as one. Anything else is written as
    /// a fraction, because no decimal would be exact.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        if self.den.is_one() {
            return write!(f, "{sign}{}", self.num.to_decimal());
        }

        let (rest, twos) = self.den.factor_out(2);
        let (rest, fives) = rest.factor_out(5);
        if !rest.is_one() {
            return write!(f, "{sign}{}/{}", self.num.to_decimal(), self.den.to_decimal());
        }

        // Scale up to a denominator of exactly 10^places.
        let places = twos.max(fives);
        let extra = BigUint::pow_small(2, places - twos).mul(&BigUint::pow_small(5, places - fives));
        let digits = self.num.mul(&extra).to_decimal();

        let places = places as usize;
        let padded = if digits.len() <= places {
            format!("{}{}", "0".repeat(places - digits.len() + 1), digits)
        } else {
            digits
        };
        let split = padded.len() - places;
        write!(f, "{sign}{}.{}", &padded[..split], &padded[split..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Rational {
        Rational::parse(s).unwrap_or_else(|| panic!("{s} should parse"))
    }

    #[test]
    fn exactness_is_the_whole_point() {
        // The canonical float embarrassment, got right.
        assert_eq!(r("0.1").add(&r("0.2")), r("0.3"));
        // And the one rationals fix that decimals cannot.
        assert_eq!(r("1/3").mul(&r("3")), r("1"));
        assert_eq!(r("1").div(&r("3")).unwrap().mul(&r("3")), r("1"));
    }

    #[test]
    fn parses_every_form() {
        assert_eq!(r("3").to_string(), "3");
        assert_eq!(r("-3").to_string(), "-3");
        assert_eq!(r("1.5").to_string(), "1.5");
        assert_eq!(r("-2.25").to_string(), "-2.25");
        assert_eq!(r("1/3").to_string(), "1/3");
        assert_eq!(r("-2/4").to_string(), "-0.5");
        assert_eq!(r("0.5").to_string(), "0.5");
        assert_eq!(r(".5").to_string(), "0.5");
        assert_eq!(r("1_000.5").to_string(), "1000.5");
    }

    #[test]
    fn rejects_nonsense() {
        for s in ["", "-", "1.2.3", "a", "1/", "/3", "1/0", "1.5/2"] {
            assert!(Rational::parse(s).is_none(), "{s} should not parse");
        }
    }

    #[test]
    fn is_always_in_lowest_terms() {
        assert_eq!(r("2/4"), r("1/2"));
        assert_eq!(r("100/10"), r("10"));
        assert_eq!(r("6/3").to_string(), "2");
    }

    #[test]
    fn prints_a_terminating_decimal_when_there_is_one() {
        assert_eq!(r("1/2").to_string(), "0.5");
        assert_eq!(r("1/4").to_string(), "0.25");
        assert_eq!(r("1/8").to_string(), "0.125");
        assert_eq!(r("1/10").to_string(), "0.1");
        assert_eq!(r("1/20").to_string(), "0.05");
        // And a fraction when there is not.
        assert_eq!(r("1/3").to_string(), "1/3");
        assert_eq!(r("22/7").to_string(), "22/7");
    }

    #[test]
    fn printing_round_trips() {
        for s in ["0", "1", "-1", "0.1", "0.125", "1/3", "-22/7", "1000000.0001"] {
            let v = r(s);
            assert_eq!(r(&v.to_string()), v, "{s} did not survive a round trip");
        }
    }

    #[test]
    fn arithmetic_across_signs() {
        assert_eq!(r("1").sub(&r("3")), r("-2"));
        assert_eq!(r("-1").add(&r("3")), r("2"));
        assert_eq!(r("-1").add(&r("-3")), r("-4"));
        assert_eq!(r("-2").mul(&r("-3")), r("6"));
        assert_eq!(r("-6").div(&r("3")).unwrap(), r("-2"));
        assert!(r("1").sub(&r("1")).is_zero());
        assert!(!r("1").sub(&r("1")).is_negative(), "zero is never negative");
    }

    #[test]
    fn refuses_to_divide_by_zero() {
        assert!(r("1").div(&r("0")).is_none());
        assert!(r("1").rem(&r("0")).is_none());
    }

    #[test]
    fn remainder_matches_the_integer_convention() {
        // Truncated, like the integer types: the sign follows the dividend.
        assert_eq!(r("7").rem(&r("3")).unwrap(), r("1"));
        assert_eq!(r("-7").rem(&r("3")).unwrap(), r("-1"));
        assert_eq!(r("7").rem(&r("-3")).unwrap(), r("1"));
        assert_eq!(r("1/2").rem(&r("1/3")).unwrap(), r("1/6"));
    }

    #[test]
    fn orders_correctly() {
        assert_eq!(r("1/3").cmp_to(&r("1/2")), Ordering::Less);
        assert_eq!(r("-1/3").cmp_to(&r("-1/2")), Ordering::Greater);
        assert_eq!(r("-1").cmp_to(&r("1")), Ordering::Less);
        assert_eq!(r("2/4").cmp_to(&r("1/2")), Ordering::Equal);
        assert_eq!(r("0").cmp_to(&r("0")), Ordering::Equal);
    }

    #[test]
    fn is_unbounded() {
        // Nothing here overflows, however far it is pushed.
        let mut v = Rational::one();
        let two = r("2");
        for _ in 0..200 {
            v = v.mul(&two);
        }
        assert_eq!(v.to_string().len(), 61, "2^200 has 61 digits");
        assert_eq!(v.div(&r("2")).unwrap().mul(&two), v);
    }

    #[test]
    fn precision_does_not_decay_the_way_a_float_would() {
        // A tenth added a thousand times is exactly one hundred.
        let tenth = r("0.1");
        let mut sum = Rational::zero();
        for _ in 0..1000 {
            sum = sum.add(&tenth);
        }
        assert_eq!(sum, r("100"));
    }
}

//! Arbitrary-precision unsigned integers.
//!
//! Limbs are `u32`, little-endian, with no trailing zeros — so an empty limb
//! vector is zero and every value has exactly one representation. Products fit
//! in `u64`, which keeps every intermediate inside a primitive and the whole
//! module free of `unsafe`.

use std::cmp::Ordering;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BigUint {
    /// Least significant limb first. Never ends in a zero limb.
    limbs: Vec<u32>,
}

impl BigUint {
    pub fn zero() -> Self {
        BigUint { limbs: Vec::new() }
    }

    pub fn one() -> Self {
        BigUint { limbs: vec![1] }
    }

    pub fn from_u64(v: u64) -> Self {
        let mut limbs = vec![v as u32, (v >> 32) as u32];
        trim(&mut limbs);
        BigUint { limbs }
    }

    pub fn from_limbs(limbs: Vec<u32>) -> Self {
        let mut limbs = limbs;
        trim(&mut limbs);
        BigUint { limbs }
    }

    pub fn limbs(&self) -> &[u32] {
        &self.limbs
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub fn is_one(&self) -> bool {
        self.limbs == [1]
    }

    /// The value as a `u64`, if it fits.
    pub fn to_u64(&self) -> Option<u64> {
        match self.limbs.len() {
            0 => Some(0),
            1 => Some(self.limbs[0] as u64),
            2 => Some(self.limbs[0] as u64 | ((self.limbs[1] as u64) << 32)),
            _ => None,
        }
    }

    /// Position of the highest set bit, plus one. Zero has zero bits.
    pub fn bits(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(top) => (self.limbs.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }

    pub fn cmp_to(&self, other: &BigUint) -> Ordering {
        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }
        // Same length, so compare from the most significant limb down.
        for i in (0..self.limbs.len()).rev() {
            match self.limbs[i].cmp(&other.limbs[i]) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        Ordering::Equal
    }

    pub fn add(&self, other: &BigUint) -> BigUint {
        let mut out = Vec::with_capacity(self.limbs.len().max(other.limbs.len()) + 1);
        let mut carry = 0u64;
        for i in 0..self.limbs.len().max(other.limbs.len()) {
            let a = *self.limbs.get(i).unwrap_or(&0) as u64;
            let b = *other.limbs.get(i).unwrap_or(&0) as u64;
            let sum = a + b + carry;
            out.push(sum as u32);
            carry = sum >> 32;
        }
        if carry != 0 {
            out.push(carry as u32);
        }
        BigUint::from_limbs(out)
    }

    /// `self - other`, or `None` if that would go below zero.
    pub fn sub(&self, other: &BigUint) -> Option<BigUint> {
        if self.cmp_to(other) == Ordering::Less {
            return None;
        }
        let mut out = Vec::with_capacity(self.limbs.len());
        let mut borrow = 0i64;
        for i in 0..self.limbs.len() {
            let a = self.limbs[i] as i64;
            let b = *other.limbs.get(i).unwrap_or(&0) as i64;
            let mut diff = a - b - borrow;
            if diff < 0 {
                diff += 1 << 32;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(diff as u32);
        }
        Some(BigUint::from_limbs(out))
    }

    pub fn mul(&self, other: &BigUint) -> BigUint {
        if self.is_zero() || other.is_zero() {
            return BigUint::zero();
        }
        let mut out = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in other.limbs.iter().enumerate() {
                let idx = i + j;
                let cur = out[idx] as u64 + a as u64 * b as u64 + carry;
                out[idx] = cur as u32;
                carry = cur >> 32;
            }
            let mut idx = i + other.limbs.len();
            while carry != 0 {
                let cur = out[idx] as u64 + carry;
                out[idx] = cur as u32;
                carry = cur >> 32;
                idx += 1;
            }
        }
        BigUint::from_limbs(out)
    }

    pub fn shl(&self, n: usize) -> BigUint {
        if self.is_zero() {
            return BigUint::zero();
        }
        let (limb_shift, bit_shift) = (n / 32, n % 32);
        let mut out = vec![0u32; limb_shift];
        let mut carry = 0u32;
        for &limb in &self.limbs {
            if bit_shift == 0 {
                out.push(limb);
            } else {
                out.push((limb << bit_shift) | carry);
                carry = (limb >> (32 - bit_shift)) as u32;
            }
        }
        if carry != 0 {
            out.push(carry);
        }
        BigUint::from_limbs(out)
    }

    pub fn shr(&self, n: usize) -> BigUint {
        let (limb_shift, bit_shift) = (n / 32, n % 32);
        if limb_shift >= self.limbs.len() {
            return BigUint::zero();
        }
        let mut out = Vec::with_capacity(self.limbs.len() - limb_shift);
        for i in limb_shift..self.limbs.len() {
            let mut v = self.limbs[i] >> bit_shift;
            if bit_shift > 0 {
                if let Some(&next) = self.limbs.get(i + 1) {
                    v |= next << (32 - bit_shift);
                }
            }
            out.push(v);
        }
        BigUint::from_limbs(out)
    }

    /// Quotient and remainder. `None` when dividing by zero.
    ///
    /// Binary long division: one shift-and-subtract per bit. Slower than
    /// estimating a whole limb at a time, and far easier to be sure of.
    pub fn divmod(&self, divisor: &BigUint) -> Option<(BigUint, BigUint)> {
        if divisor.is_zero() {
            return None;
        }
        if self.cmp_to(divisor) == Ordering::Less {
            return Some((BigUint::zero(), self.clone()));
        }

        let shift = self.bits() - divisor.bits();
        let mut window = divisor.shl(shift);
        let mut rem = self.clone();
        let mut quotient = vec![0u32; shift / 32 + 1];

        for i in (0..=shift).rev() {
            if window.cmp_to(&rem) != Ordering::Greater {
                rem = rem.sub(&window).expect("window is no greater than the remainder");
                quotient[i / 32] |= 1 << (i % 32);
            }
            window = window.shr(1);
        }
        Some((BigUint::from_limbs(quotient), rem))
    }

    /// Divide by a value that fits in a limb, which display and parsing need.
    fn divmod_small(&self, divisor: u32) -> (BigUint, u32) {
        let mut out = vec![0u32; self.limbs.len()];
        let mut rem = 0u64;
        for i in (0..self.limbs.len()).rev() {
            let cur = (rem << 32) | self.limbs[i] as u64;
            out[i] = (cur / divisor as u64) as u32;
            rem = cur % divisor as u64;
        }
        (BigUint::from_limbs(out), rem as u32)
    }

    fn mul_small_add(&self, factor: u32, addend: u32) -> BigUint {
        let mut out = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry = addend as u64;
        for &limb in &self.limbs {
            let cur = limb as u64 * factor as u64 + carry;
            out.push(cur as u32);
            carry = cur >> 32;
        }
        while carry != 0 {
            out.push(carry as u32);
            carry >>= 32;
        }
        BigUint::from_limbs(out)
    }

    pub fn gcd(&self, other: &BigUint) -> BigUint {
        let mut a = self.clone();
        let mut b = other.clone();
        while !b.is_zero() {
            let (_, r) = a.divmod(&b).expect("b is not zero");
            a = b;
            b = r;
        }
        a
    }

    /// Remove every factor of `factor`, returning how many came out.
    pub fn factor_out(&self, factor: u32) -> (BigUint, u32) {
        let mut v = self.clone();
        let mut count = 0;
        loop {
            let (q, r) = v.divmod_small(factor);
            if r != 0 || v.is_zero() {
                return (v, count);
            }
            v = q;
            count += 1;
        }
    }

    pub fn pow_small(base: u32, exp: u32) -> BigUint {
        let mut out = BigUint::one();
        for _ in 0..exp {
            out = out.mul_small_add(base, 0);
        }
        out
    }

    pub fn parse_decimal(text: &str) -> Option<BigUint> {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut out = BigUint::zero();
        for b in text.bytes() {
            out = out.mul_small_add(10, (b - b'0') as u32);
        }
        Some(out)
    }

    pub fn to_decimal(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let mut digits = Vec::new();
        let mut v = self.clone();
        while !v.is_zero() {
            // Nine digits at a time: the largest power of ten inside a limb.
            let (q, r) = v.divmod_small(1_000_000_000);
            digits.push(r);
            v = q;
        }
        let mut out = digits.pop().expect("non-zero").to_string();
        while let Some(chunk) = digits.pop() {
            out.push_str(&format!("{chunk:09}"));
        }
        out
    }
}

fn trim(limbs: &mut Vec<u32>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> BigUint {
        BigUint::parse_decimal(s).unwrap()
    }

    #[test]
    fn round_trips_decimal() {
        for s in ["0", "1", "9", "10", "4294967295", "4294967296", "18446744073709551616",
                  "123456789012345678901234567890"] {
            assert_eq!(n(s).to_decimal(), s);
        }
    }

    #[test]
    fn adds_across_limb_boundaries() {
        assert_eq!(n("4294967295").add(&n("1")).to_decimal(), "4294967296");
        assert_eq!(n("18446744073709551615").add(&n("1")).to_decimal(), "18446744073709551616");
    }

    #[test]
    fn subtracts_and_refuses_to_go_negative() {
        assert_eq!(n("4294967296").sub(&n("1")).unwrap().to_decimal(), "4294967295");
        assert!(n("1").sub(&n("2")).is_none());
        assert!(n("5").sub(&n("5")).unwrap().is_zero());
    }

    #[test]
    fn multiplies() {
        assert_eq!(n("123456789").mul(&n("987654321")).to_decimal(), "121932631112635269");
        assert_eq!(n("0").mul(&n("999")).to_decimal(), "0");
        let big = n("18446744073709551616");
        assert_eq!(big.mul(&big).to_decimal(), "340282366920938463463374607431768211456");
    }

    #[test]
    fn divides_with_remainder() {
        let (q, r) = n("121932631112635269").divmod(&n("987654321")).unwrap();
        assert_eq!(q.to_decimal(), "123456789");
        assert!(r.is_zero());

        let (q, r) = n("100").divmod(&n("7")).unwrap();
        assert_eq!((q.to_decimal(), r.to_decimal()), ("14".into(), "2".into()));

        let (q, r) = n("3").divmod(&n("10")).unwrap();
        assert!(q.is_zero());
        assert_eq!(r.to_decimal(), "3");
    }

    #[test]
    fn refuses_to_divide_by_zero() {
        assert!(n("1").divmod(&n("0")).is_none());
    }

    #[test]
    fn division_inverts_multiplication_over_many_sizes() {
        for a in ["7", "4294967296", "123456789012345678901234567890"] {
            for b in ["3", "1000000007", "18446744073709551616"] {
                let (x, y) = (n(a), n(b));
                let product = x.mul(&y);
                let (q, r) = product.divmod(&y).unwrap();
                assert!(r.is_zero(), "{a} * {b} / {b} left a remainder");
                assert_eq!(q, x, "{a} * {b} / {b} did not come back");
            }
        }
    }

    #[test]
    fn shifts() {
        assert_eq!(n("1").shl(64).to_decimal(), "18446744073709551616");
        assert_eq!(n("18446744073709551616").shr(64).to_decimal(), "1");
        assert_eq!(n("5").shl(1).to_decimal(), "10");
        assert_eq!(n("5").shr(1).to_decimal(), "2");
        assert!(n("1").shr(100).is_zero());
    }

    #[test]
    fn gcd_is_the_greatest_common_divisor() {
        assert_eq!(n("12").gcd(&n("18")).to_decimal(), "6");
        assert_eq!(n("17").gcd(&n("5")).to_decimal(), "1");
        assert_eq!(n("0").gcd(&n("7")).to_decimal(), "7");
        assert_eq!(n("123456789012345678890").gcd(&n("10")).to_decimal(), "10");
    }

    #[test]
    fn factors_out_powers() {
        let (rest, twos) = n("80").factor_out(2);
        assert_eq!((rest.to_decimal(), twos), ("5".into(), 4));
        let (rest, fives) = n("1000").factor_out(5);
        assert_eq!((rest.to_decimal(), fives), ("8".into(), 3));
        let (rest, threes) = n("7").factor_out(3);
        assert_eq!((rest.to_decimal(), threes), ("7".into(), 0));
    }

    #[test]
    fn bits_counts_correctly() {
        assert_eq!(n("0").bits(), 0);
        assert_eq!(n("1").bits(), 1);
        assert_eq!(n("255").bits(), 8);
        assert_eq!(n("256").bits(), 9);
        assert_eq!(n("18446744073709551616").bits(), 65);
    }
}

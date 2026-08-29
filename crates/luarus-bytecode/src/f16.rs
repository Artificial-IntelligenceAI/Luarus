//! Minimal IEEE 754 binary16 support.
//!
//! Rust has no stable `f16`, so Luarus stores half-precision values as raw
//! `u16` bits and computes on them in `f32`, rounding back to half precision
//! after every operation. `f16` therefore really does lose precision the way
//! the type promises, rather than being `f32` under a different name.

/// Round-to-nearest-even conversion from `f32` to binary16 bits.
pub fn from_f32(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x007f_ffff;

    if exp == 0xff {
        // Infinity, or NaN with a mantissa preserved as non-zero.
        let m = if mantissa != 0 { 0x0200 } else { 0 };
        return sign | 0x7c00 | m;
    }

    let unbiased = exp - 127 + 15;
    if unbiased >= 0x1f {
        return sign | 0x7c00; // overflows half precision
    }
    if unbiased <= 0 {
        if unbiased < -10 {
            return sign; // underflows to zero
        }
        // Subnormal: reintroduce the implicit leading bit, then shift.
        let m = mantissa | 0x0080_0000;
        let shift = (14 - unbiased) as u32;
        let mut half = (m >> shift) as u16;
        let round_bit = 1u32 << (shift - 1);
        if (m & round_bit) != 0 && (m & (3 * round_bit - 1)) != 0 {
            half += 1;
        }
        return sign | half;
    }

    let mut half = sign | ((unbiased as u16) << 10) | ((mantissa >> 13) as u16);
    // Round to nearest, ties to even.
    if (mantissa & 0x1fff) > 0x1000 || ((mantissa & 0x1fff) == 0x1000 && (mantissa & 0x2000) != 0) {
        half = half.wrapping_add(1);
    }
    half
}

/// Exact conversion from binary16 bits to `f32`.
pub fn to_f32(half: u16) -> f32 {
    let sign = ((half & 0x8000) as u32) << 16;
    let exp = ((half >> 10) & 0x1f) as u32;
    let mantissa = (half & 0x03ff) as u32;

    if exp == 0 {
        if mantissa == 0 {
            return f32::from_bits(sign);
        }
        // Subnormal half: renormalise into a normal f32.
        let mut m = mantissa;
        let mut e = -1i32;
        while (m & 0x0400) == 0 {
            m <<= 1;
            e -= 1;
        }
        m &= 0x03ff;
        let exp32 = ((e + 1 - 15 + 127) as u32) << 23;
        return f32::from_bits(sign | exp32 | (m << 13));
    }
    if exp == 0x1f {
        return f32::from_bits(sign | 0x7f80_0000 | (mantissa << 13));
    }
    f32::from_bits(sign | ((exp + 127 - 15) << 23) | (mantissa << 13))
}

/// Snap an `f32` to the nearest value representable in half precision.
pub fn round(value: f32) -> f32 {
    to_f32(from_f32(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_exact_values() {
        for v in [0.0f32, 1.0, -1.0, 0.5, 1000.0, -2048.0] {
            assert_eq!(round(v), v, "{v} should be exact in f16");
        }
    }

    #[test]
    fn loses_precision_like_real_half() {
        // 2049 is not representable in binary16; it rounds to 2048.
        assert_eq!(round(2049.0), 2048.0);
        // 65520 and above overflow to infinity.
        assert!(round(70000.0).is_infinite());
    }

    #[test]
    fn preserves_sign_of_zero() {
        assert!(round(-0.0f32).is_sign_negative());
    }
}

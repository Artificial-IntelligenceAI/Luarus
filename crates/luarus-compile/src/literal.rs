//! Turning a quoted literal into a constant of a known type.
//!
//! In Luarus a literal carries no type of its own: `'1000'` is an integer, a
//! float, or the text "1000" depending entirely on the type it is checked
//! against. All of the parsing therefore lives here, driven by that type.

use luarus_bytecode::{f16, Const, RtType};
use luarus_diag::Rule;

#[derive(Debug)]
pub struct LiteralError {
    pub rule: Rule,
    pub message: String,
    pub help: Option<String>,
}

fn err(message: impl Into<String>) -> LiteralError {
    LiteralError { rule: Rule::ValuesMustFit, message: message.into(), help: None }
}

fn err_help(message: impl Into<String>, help: impl Into<String>) -> LiteralError {
    LiteralError {
        rule: Rule::ValuesMustFit,
        message: message.into(),
        help: Some(help.into()),
    }
}

/// Interpret `text` as a value of type `ty`.
pub fn parse(text: &str, ty: RtType) -> Result<Const, LiteralError> {
    match ty {
        RtType::Str => Ok(Const::Str(text.to_string())),
        RtType::Bool => match text {
            "true" => Ok(Const::Bool(true)),
            "false" => Ok(Const::Bool(false)),
            _ => Err(err_help(
                format!("`'{text}'` is not a bool"),
                "a bool literal is written `'true'` or `'false'`",
            )),
        },
        RtType::Nil => {
            if text == "nil" {
                Ok(Const::Nil)
            } else {
                Err(err_help(
                    format!("`'{text}'` is not nil"),
                    "the only value of type `nil` is written `'nil'`",
                ))
            }
        }
        RtType::Er => luarus_num::Rational::parse(text).map(Const::Er).ok_or_else(|| {
            err_help(
                format!("`'{text}'` is not a valid `er`"),
                "an exact rational is written as an integer, a decimal, or a fraction: \
                 `'3'`, `'0.1'`, `'1/3'`",
            )
        }),
        t if t.is_int() => parse_int(text, t),
        t if t.is_float() => parse_float(text, t),
        _ => Err(err(format!("cannot write a literal of type `{}`", ty.name()))),
    }
}

/// Strip the `_` digit separators Luarus allows in numbers.
fn clean(text: &str) -> String {
    text.chars().filter(|c| *c != '_').collect()
}

fn parse_int(text: &str, ty: RtType) -> Result<Const, LiteralError> {
    let cleaned = clean(text);
    let body = cleaned.trim();
    if body.is_empty() {
        return Err(err(format!("empty literal is not a valid `{}`", ty.name())));
    }

    let (neg, digits) = match body.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, body.strip_prefix('+').unwrap_or(body)),
    };

    // Radix prefixes make bit-level constants readable without a second syntax.
    let (radix, digits) = if let Some(d) = digits.strip_prefix("0x").or(digits.strip_prefix("0X")) {
        (16, d)
    } else if let Some(d) = digits.strip_prefix("0b").or(digits.strip_prefix("0B")) {
        (2, d)
    } else if let Some(d) = digits.strip_prefix("0o").or(digits.strip_prefix("0O")) {
        (8, d)
    } else {
        (10, digits)
    };

    if digits.is_empty() {
        return Err(err(format!("`'{text}'` is not a valid `{}`", ty.name())));
    }

    let magnitude = i128::from_str_radix(digits, radix).map_err(|_| {
        if text.contains('.') {
            err_help(
                format!("`'{text}'` is not a valid `{}`", ty.name()),
                format!("`{}` is an integer type; use `f64` for a fractional value", ty.name()),
            )
        } else {
            err(format!("`'{text}'` is not a valid `{}`", ty.name()))
        }
    })?;

    let value = if neg { -magnitude } else { magnitude };
    let (lo, hi) = ty.int_range().expect("integer type has a range");
    if value < lo || value > hi {
        let help = if neg && ty.is_unsigned_int() {
            format!("`{}` is unsigned and cannot hold a negative value", ty.name())
        } else {
            format!("`{}` holds values from {lo} to {hi}", ty.name())
        };
        return Err(err_help(format!("`{value}` is out of range for `{}`", ty.name()), help));
    }

    if ty.is_unsigned_int() {
        Ok(Const::Uint(value as u64))
    } else {
        Ok(Const::Int(value as i64))
    }
}

fn parse_float(text: &str, ty: RtType) -> Result<Const, LiteralError> {
    let cleaned = clean(text);
    let body = cleaned.trim();
    let value: f64 = match body {
        "inf" | "+inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        "nan" => f64::NAN,
        _ => body
            .parse()
            .map_err(|_| err(format!("`'{text}'` is not a valid `{}`", ty.name())))?,
    };

    match ty {
        RtType::F64 => Ok(Const::F64(value)),
        RtType::F32 => {
            let narrowed = value as f32;
            if narrowed.is_infinite() && value.is_finite() {
                return Err(err_help(
                    format!("`{value}` is out of range for `f32`"),
                    "`f32` holds magnitudes up to about 3.4e38; use `f64` instead",
                ));
            }
            Ok(Const::F32(narrowed))
        }
        RtType::F16 => {
            let narrowed = f16::from_f32(value as f32);
            if f16::to_f32(narrowed).is_infinite() && value.is_finite() {
                return Err(err_help(
                    format!("`{value}` is out of range for `f16`"),
                    "`f16` holds magnitudes up to 65504; use `f32` or `f64` instead",
                ));
            }
            Ok(Const::F16(narrowed))
        }
        _ => unreachable!("parse_float called with {}", ty.name()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_the_same_text_differently() {
        assert_eq!(parse("1000", RtType::I32).map_err(|_| ()), Ok(Const::Int(1000)));
        assert_eq!(parse("1000", RtType::Str).map_err(|_| ()), Ok(Const::Str("1000".into())));
        assert!(matches!(parse("1000", RtType::F16).unwrap(), Const::F16(_)));
    }

    #[test]
    fn rejects_out_of_range_integers() {
        assert!(parse("300", RtType::U8).is_err());
        assert!(parse("-1", RtType::U32).is_err());
        assert!(parse("127", RtType::I8).is_ok());
        assert!(parse("128", RtType::I8).is_err());
    }

    #[test]
    fn accepts_separators_and_radixes() {
        assert_eq!(parse("1_000_000", RtType::I32).map_err(|_| ()), Ok(Const::Int(1_000_000)));
        assert_eq!(parse("0xff", RtType::U8).map_err(|_| ()), Ok(Const::Uint(255)));
        assert_eq!(parse("0b1010", RtType::I32).map_err(|_| ()), Ok(Const::Int(10)));
    }
}

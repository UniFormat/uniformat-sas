//! IBM hexadecimal floating point, derived from its mathematical definition.
use super::invalid;
use crate::{MissingKind, NumValue, Result};

/// Decode an IBM number of 2–8 bytes, including SAS tagged missing values.
/// The integer-to-f64 conversion rounds to nearest, ties to even; power-of-two
/// scaling is exact throughout the IBM range (no IEEE subnormals or overflow).
pub fn decode_number(input: &[u8]) -> Result<NumValue> {
    if !(2..=8).contains(&input.len()) {
        return Err(invalid("XPT numeric width (2..8)"));
    }
    let mut buf = [0u8; 8];
    super::put(&mut buf, 0, input)?;
    let word = u64::from_be_bytes(buf);
    let tag = (word >> 56) as u8;
    let mantissa = word & 0x00ff_ffff_ffff_ffff;
    if mantissa == 0 {
        let missing = match tag {
            b'.' => Some(MissingKind::Dot),
            b'_' => Some(MissingKind::Underscore),
            b'A'..=b'Z' => Some(MissingKind::Letter(tag)),
            _ => None,
        };
        if let Some(m) = missing {
            return Ok(NumValue::Missing(m));
        }
        return Ok(NumValue::Value(0.0));
    }
    let exponent = i32::from(tag & 0x7f)
        .saturating_sub(64)
        .saturating_mul(4)
        .saturating_sub(56);
    let sign = if tag & 0x80 != 0 { -1.0 } else { 1.0 };
    Ok(NumValue::Value(
        sign * (mantissa as f64) * 2.0_f64.powi(exponent),
    ))
}

/// Encode a number exactly as an eight-byte normalized IBM number.
/// NaN becomes `.`, infinity/underflow/overflow are errors, signed zero is canonicalized.
/// No floating-point logarithm is used: the binary exponent determines hexadecimal normalization.
pub fn encode_number(value: NumValue) -> Result<[u8; 8]> {
    let value = match value {
        NumValue::Missing(m) => {
            let tag = match m {
                MissingKind::Dot => b'.',
                MissingKind::Underscore => b'_',
                MissingKind::Letter(c @ b'A'..=b'Z') => c,
                MissingKind::Letter(_) => return Err(invalid("XPT missing tag (A..Z)")),
            };
            return Ok((u64::from(tag) << 56).to_be_bytes());
        }
        NumValue::Value(v) if v.is_nan() => {
            return encode_number(NumValue::Missing(MissingKind::Dot))
        }
        NumValue::Value(v) => v,
    };
    if value == 0.0 {
        return Ok([0; 8]);
    }
    let bits = value.to_bits();
    let binary_exp = (((bits >> 52) & 0x7ff) as i32).saturating_sub(1023);
    if !value.is_finite() || !(-260..252).contains(&binary_exp) {
        return Err(invalid("XPT number outside normalized IBM range"));
    }
    let hex_exp = binary_exp.div_euclid(4).saturating_add(65);
    let shift = binary_exp.rem_euclid(4) as u32;
    let mantissa = ((bits & 0x000f_ffff_ffff_ffff) | (1 << 52)) << shift;
    let word = (bits & (1 << 63)) | ((hex_exp as u64) << 56) | mantissa;
    Ok(word.to_be_bytes())
}

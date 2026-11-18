//! Validated, reproducible transport timestamps; fixed century pivot 1969/2068.
use super::{bytes, decimal, invalid};
use crate::Result;

pub(super) fn validate_date(input: &[u8]) -> Result<()> {
    if input.len() != 16 {
        return Err(invalid("XPT timestamp width"));
    }
    if input.iter().all(|&b| b == b' ') {
        return Ok(());
    }
    if [7, 10, 13].iter().any(|&i| input.get(i) != Some(&b':')) {
        return Err(invalid("XPT timestamp separator"));
    }
    let day = decimal(bytes(input, 0, 2)?)?;
    let month = [
        b"JAN", b"FEB", b"MAR", b"APR", b"MAY", b"JUN", b"JUL", b"AUG", b"SEP", b"OCT", b"NOV",
        b"DEC",
    ]
    .iter()
    .position(|m| m.as_slice() == bytes(input, 2, 3).unwrap_or_default())
    .ok_or_else(|| invalid("XPT timestamp month"))?;
    let year = decimal(bytes(input, 5, 2)?)?;
    let year = if year >= 69 {
        year.saturating_add(1900)
    } else {
        year.saturating_add(2000)
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ]
    .get(month)
    .copied()
    .unwrap_or(0);
    if day == 0
        || day > max_day
        || decimal(bytes(input, 8, 2)?)? > 23
        || decimal(bytes(input, 11, 2)?)? > 59
        || decimal(bytes(input, 14, 2)?)? > 59
    {
        return Err(invalid("XPT timestamp range"));
    }
    Ok(())
}

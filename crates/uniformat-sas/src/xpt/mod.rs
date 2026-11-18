//! SAS Transport v5, buffers only. See `docs/spec/xpt.md` and ADR 0008.
mod date;
mod float;
mod header;
mod namestr;
mod row;
mod text;
mod write;
mod write_schema;

pub use float::{decode_number, encode_number};
pub use header::{
    detect_version, is_member_boundary, parse_library, parse_member, parse_observation_header,
    MemberHeader, Metadata, Version,
};
pub use namestr::parse_namestrs;
pub use row::Row;
pub use text::TextEncoding;
pub use write::{Encoder, WriteOptions};

use crate::{Error, Result};

/// Physical record width in bytes.
pub const RECORD_LEN: usize = 80;

pub(crate) fn invalid(what: &'static str) -> Error {
    Error::CorruptMetadata { what }
}

pub(crate) fn bytes(input: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| invalid("XPT offset overflow"))?;
    input.get(offset..end).ok_or(Error::Truncated {
        needed: end as u64,
        got: input.len() as u64,
    })
}

pub(crate) fn put(dst: &mut [u8], offset: usize, src: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(src.len())
        .ok_or_else(|| invalid("XPT offset overflow"))?;
    dst.get_mut(offset..end)
        .ok_or_else(|| invalid("XPT output buffer"))?
        .copy_from_slice(src);
    Ok(())
}

pub(crate) fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    let b = bytes(input, offset, 2)?;
    Ok(u16::from_be_bytes(
        b.try_into().map_err(|_| invalid("XPT integer"))?,
    ))
}

pub(crate) fn decimal(input: &[u8]) -> Result<usize> {
    if input.is_empty() {
        return Err(invalid("XPT decimal field"));
    }
    input.iter().try_fold(0usize, |v, &b| {
        if !b.is_ascii_digit() {
            return Err(invalid("XPT decimal field"));
        }
        v.checked_mul(10)
            .and_then(|v| v.checked_add(usize::from(b.saturating_sub(b'0'))))
            .ok_or_else(|| invalid("XPT decimal overflow"))
    })
}

/// Bytes needed to align a block to the next physical record.
#[must_use]
pub fn padding(len: usize) -> usize {
    RECORD_LEN.saturating_sub(len % RECORD_LEN) % RECORD_LEN
}

/// Infer the count of observations using the largest valid padding (0–79 spaces).
/// Trailing all-blank short observations are intrinsically ambiguous in v5.
/// `tail` must contain the last `min(block_len, 79)` bytes of the data block.
pub fn observation_count(block_len: u64, row_len: u32, tail: &[u8]) -> Result<u64> {
    if row_len == 0 || !block_len.is_multiple_of(80) || tail.len() != block_len.min(79) as usize {
        return Err(invalid("XPT data block length"));
    }
    let spaces = tail.iter().rev().take_while(|&&b| b == b' ').count();
    for pad in (0..=spaces).rev() {
        let data_len = block_len.saturating_sub(pad as u64);
        if data_len.is_multiple_of(u64::from(row_len)) {
            return data_len
                .checked_div(u64::from(row_len))
                .ok_or_else(|| invalid("XPT zero row width"));
        }
    }
    Err(invalid("XPT truncated observation or non-space padding"))
}

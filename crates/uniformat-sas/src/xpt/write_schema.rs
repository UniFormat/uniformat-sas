//! Schema validation and NAMESTR serialization before any output I/O.
use super::{invalid, padding, put, TextEncoding, WriteOptions};
use crate::write::{WriteKind, WriteSchema};
use crate::{Limits, Result, SasFormat};
use std::collections::HashSet;

pub(super) fn record(name: &str) -> Vec<u8> {
    format!("HEADER RECORD*******{name:<8}HEADER RECORD!!!!!!!000000000000000000000000000000  ")
        .into_bytes()
}
pub(super) fn field(
    dst: &mut [u8],
    offset: usize,
    width: usize,
    text: &str,
    enc: TextEncoding,
) -> Result<()> {
    let src = enc.encode(text)?;
    if src.len() > width {
        return Err(invalid("XPT text exceeds field width"));
    }
    put(dst, offset, &src)
}
pub(super) fn name(text: &str) -> Result<()> {
    let mut chars = text.bytes();
    if text.len() > 8
        || !chars
            .next()
            .is_some_and(|c| c == b'_' || c.is_ascii_alphabetic())
        || !chars.all(|c| c == b'_' || c.is_ascii_alphanumeric())
    {
        return Err(invalid(
            "XPT v5 name must be an ASCII identifier of 1..8 bytes",
        ));
    }
    Ok(())
}
fn format(dst: &mut [u8], offset: usize, fmt: &Option<SasFormat>, enc: TextEncoding) -> Result<()> {
    put(dst, offset, b"        ")?;
    if let Some(f) = fmt {
        if !f.name.is_ascii() || f.width > i16::MAX as u16 || f.decimals > i16::MAX as u16 {
            return Err(invalid("XPT format name, width or decimals"));
        }
        field(dst, offset, 8, &f.name, enc)?;
        put(dst, offset.saturating_add(8), &f.width.to_be_bytes())?;
        put(dst, offset.saturating_add(10), &f.decimals.to_be_bytes())?;
    }
    Ok(())
}
pub(super) fn descriptors(
    schema: &WriteSchema,
    opts: &WriteOptions,
    limits: &Limits,
) -> Result<Vec<u8>> {
    let n = schema.columns.len();
    let size = n
        .checked_mul(140)
        .filter(|&s| s <= limits.max_text_bytes)
        .ok_or_else(|| invalid("XPT metadata budget"))?;
    if n == 0 || n > 9999 || n > limits.max_columns as usize {
        return Err(invalid("XPT column count"));
    }
    let mut out = Vec::with_capacity(size.saturating_add(padding(size)));
    let mut names = HashSet::with_capacity(n);
    let mut position = 0u32;
    for (i, c) in schema.columns.iter().enumerate() {
        name(&c.name)?;
        if !names.insert(c.name.to_ascii_uppercase()) {
            return Err(invalid("XPT duplicate name"));
        }
        let (kind, width) = match c.kind {
            WriteKind::Numeric { length: n @ 2..=8 } => (1u16, u16::from(n)),
            WriteKind::Char {
                length: n @ 1..=200,
            } => (2u16, n as u16),
            _ => return Err(invalid("XPT v5 column width")),
        };
        let mut d = [0u8; 140];
        put(&mut d, 0, &kind.to_be_bytes())?;
        put(&mut d, 4, &width.to_be_bytes())?;
        put(&mut d, 6, &((i.saturating_add(1)) as u16).to_be_bytes())?;
        put(&mut d, 8, &[b' '; 48])?;
        field(&mut d, 8, 8, &c.name, opts.encoding)?;
        field(&mut d, 16, 40, &c.label, opts.encoding)?;
        format(&mut d, 56, &c.format, opts.encoding)?;
        format(&mut d, 72, &c.informat, opts.encoding)?;
        put(&mut d, 84, &position.to_be_bytes())?;
        position = position
            .checked_add(u32::from(width))
            .filter(|&n| n <= limits.max_row_length)
            .ok_or_else(|| invalid("XPT observation width budget"))?;
        out.extend_from_slice(&d);
    }
    out.resize(size.saturating_add(padding(size)), b' ');
    Ok(out)
}

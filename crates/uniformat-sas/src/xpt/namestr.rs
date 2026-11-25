use super::{bytes, invalid, u16_at, TextEncoding, Version};
use crate::{classify_format, Column, ColumnKind, Limits, Result, SasFormat, StorageKind};
use std::collections::HashSet;

fn format(input: &[u8], off: usize, enc: TextEncoding) -> Result<Option<SasFormat>> {
    let name = enc.field(bytes(input, off, 8)?)?;
    let width = u16_at(input, off.saturating_add(8))?;
    let decimals = u16_at(input, off.saturating_add(10))?;
    if width > i16::MAX as u16 || decimals > i16::MAX as u16 {
        return Err(invalid("XPT negative format width or decimals"));
    }
    Ok(
        (!name.is_empty() || width != 0 || decimals != 0).then_some(SasFormat {
            name,
            width,
            decimals,
        }),
    )
}

/// Parse a bounded NAMESTR block; descriptors may have 135 or 140 bytes.
/// Returns columns and the validated observation width. Input includes block padding.
pub fn parse_namestrs(
    input: &[u8],
    count: usize,
    width: usize,
    enc: TextEncoding,
    limits: &Limits,
) -> Result<(Vec<Column>, u32)> {
    parse_version(input, count, width, enc, limits, Version::V5)
}

pub(super) fn parse_version(
    input: &[u8],
    count: usize,
    width: usize,
    enc: TextEncoding,
    limits: &Limits,
    version: Version,
) -> Result<(Vec<Column>, u32)> {
    if ![135, 140].contains(&width)
        || (version == Version::V8 && width != 140)
        || count == 0
        || count > version.columns()
        || count > limits.max_columns as usize
    {
        return Err(invalid("XPT NAMESTR count or size"));
    }
    let size = count
        .checked_mul(width)
        .ok_or_else(|| invalid("XPT NAMESTR size"))?;
    if size > limits.max_text_bytes {
        return Err(invalid("XPT metadata budget"));
    }
    let padded = size
        .checked_add(super::padding(size))
        .ok_or_else(|| invalid("XPT metadata size"))?;
    if input.len() != padded
        || bytes(input, size, padded.saturating_sub(size))?
            .iter()
            .any(|&b| b != b' ')
    {
        return Err(invalid("XPT NAMESTR padding"));
    }
    let mut columns = Vec::with_capacity(count);
    let mut names = HashSet::with_capacity(count);
    let mut ranges = Vec::with_capacity(count);
    let mut row_len = 0;
    for (index, input) in bytes(input, 0, size)?.chunks_exact(width).enumerate() {
        let length = u16_at(input, 4)?;
        let (kind, storage) = match (u16_at(input, 0)?, length) {
            (1, 2..=8) => (
                ColumnKind::Numeric {
                    length: length as u8,
                },
                StorageKind::Numeric,
            ),
            (2, n) if n > 0 && u32::from(n) <= version.char_len() => (
                ColumnKind::Char {
                    length: u32::from(length),
                },
                StorageKind::Char,
            ),
            _ => return Err(invalid("XPT variable type or width")),
        };
        if usize::from(u16_at(input, 6)?) != index.saturating_add(1) {
            return Err(invalid("XPT variable ordinal"));
        }
        let name = enc.field(bytes(
            input,
            if version == Version::V5 { 8 } else { 88 },
            version.name_len(),
        )?)?;
        if name.is_empty() || !names.insert(name.to_ascii_uppercase()) {
            return Err(invalid("XPT duplicate or empty name"));
        }
        let offset = u32::from_be_bytes(
            bytes(input, 84, 4)?
                .try_into()
                .map_err(|_| invalid("XPT position"))?,
        );
        if offset > i32::MAX as u32 || u16_at(input, 68)? > 1 {
            return Err(invalid("XPT negative position or invalid justification"));
        }
        let end = offset
            .checked_add(u32::from(length))
            .filter(|&v| v <= limits.max_row_length)
            .ok_or_else(|| invalid("XPT observation width budget"))?;
        ranges.push((offset, end));
        row_len = row_len.max(end);
        let label_len = if version == Version::V8 {
            u16_at(input, 120)?
        } else {
            40
        };
        if label_len > 256 {
            return Err(invalid("XPT label length"));
        }
        // A truncated UTF-8 prefix is not independently decodable; LABEL restores it.
        let label = if label_len > 40 {
            String::new()
        } else {
            enc.field(bytes(input, 16, 40)?)?
        };
        let format = format(input, 56, enc)?;
        let informat = self::format(input, 72, enc)?;
        let logical = classify_format(
            format
                .as_ref()
                .or(informat.as_ref())
                .map_or("", |f| &f.name),
            storage,
        );
        columns.push(Column {
            index,
            name,
            label: (!label.is_empty()).then_some(label),
            kind,
            offset,
            format,
            informat,
            indexed: false,
            logical,
        });
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|w| matches!(w, [a, b] if a.1 > b.0)) {
        return Err(invalid("XPT overlapping columns"));
    }
    Ok((columns, row_len))
}

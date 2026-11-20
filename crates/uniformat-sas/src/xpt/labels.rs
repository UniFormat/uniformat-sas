use super::formats::parse_format;
use super::{bytes, decimal, header::check_marker, invalid, padding, u16_at, Metadata, Version};
use crate::{classify_format, ColumnKind, Result, StorageKind};
use std::collections::HashSet;

/// Validated LABELV8/LABELV9 header for bounded incremental I/O.
#[derive(Debug, Clone, Copy)]
pub struct LabelHeader {
    count: usize,
    extended: bool,
}
impl LabelHeader {
    /// Recognize a label section; all other markers return None.
    pub fn parse(input: &[u8], variables: usize) -> Result<Option<Self>> {
        let extended = check_marker(input, "LABELV9").is_ok();
        if !extended && check_marker(input, "LABELV8").is_err() {
            return Ok(None);
        }
        let field = std::str::from_utf8(bytes(input, 48, 5)?)
            .map_err(|_| invalid("XPT label count"))?
            .trim_matches(' ');
        let count = decimal(field.as_bytes())?;
        if count == 0 || count > variables || bytes(input, 53, 27)?.iter().any(|&b| b != b' ') {
            return Err(invalid("XPT label count or padding"));
        }
        Ok(Some(Self { count, extended }))
    }
    /// Number of entries, bounded by the member variable count.
    pub fn count(self) -> usize {
        self.count
    }
    /// Fixed prefix size to read before requesting the variable payload.
    pub fn prefix_len(self) -> usize {
        if self.extended {
            10
        } else {
            6
        }
    }
    /// Validate lengths before deriving any allocation or read range.
    pub fn entry_len(self, prefix: &[u8]) -> Result<usize> {
        let name = u16_at(prefix, 2)?;
        let label = u16_at(prefix, 4)?;
        if name == 0 || name > 32 || label > 256 {
            return Err(invalid("XPT extended name or label length"));
        }
        let mut size = self
            .prefix_len()
            .saturating_add(usize::from(name))
            .saturating_add(usize::from(label));
        if self.extended {
            for off in [6, 8] {
                let n = u16_at(prefix, off)?;
                if n > 44 {
                    return Err(invalid("XPT extended format length"));
                }
                size = size.saturating_add(usize::from(n));
            }
        }
        Ok(size)
    }
}

pub(super) fn apply(meta: &mut Metadata, input: &[u8]) -> Result<usize> {
    let Some(h) = LabelHeader::parse(input, meta.columns.len())? else {
        if meta.label_lengths.iter().any(|&n| n > 40) {
            return Err(invalid("XPT missing long label section"));
        }
        return Ok(0);
    };
    if meta.header().version != Version::V8 {
        return Err(invalid("XPT labels require v8"));
    }
    let enc = meta.encoding();
    let mut at = 80usize;
    let mut seen = HashSet::new();
    for _ in 0..h.count() {
        let size = h.entry_len(bytes(input, at, h.prefix_len())?)?;
        let entry = bytes(input, at, size)?;
        let ordinal = usize::from(u16_at(entry, 0)?);
        let index = ordinal
            .checked_sub(1)
            .ok_or_else(|| invalid("XPT label ordinal"))?;
        if !seen.insert(index) {
            return Err(invalid("XPT duplicate label entry"));
        }
        let name_len = usize::from(u16_at(entry, 2)?);
        let label_len = usize::from(u16_at(entry, 4)?);
        if meta.label_lengths.get(index).copied() != Some(label_len as u16) {
            return Err(invalid("XPT inconsistent label length"));
        }
        let col = meta
            .columns
            .get_mut(index)
            .ok_or_else(|| invalid("XPT label ordinal"))?;
        if enc.decode(bytes(entry, h.prefix_len(), name_len)?)? != col.name {
            return Err(invalid("XPT inconsistent long name"));
        }
        let label_at = h.prefix_len().saturating_add(name_len);
        let label = enc.decode(bytes(entry, label_at, label_len)?)?.into_owned();
        col.label = (!label.is_empty()).then_some(label);
        if h.extended {
            let fmt_at = label_at.saturating_add(label_len);
            let fmt_len = usize::from(u16_at(entry, 6)?);
            let inf_len = usize::from(u16_at(entry, 8)?);
            col.format = parse_format(bytes(entry, fmt_at, fmt_len)?, &col.format)?;
            col.informat = parse_format(
                bytes(entry, fmt_at.saturating_add(fmt_len), inf_len)?,
                &col.informat,
            )?;
            let storage = match col.kind {
                ColumnKind::Numeric { .. } => StorageKind::Numeric,
                ColumnKind::Char { .. } => StorageKind::Char,
            };
            col.logical = classify_format(
                col.format
                    .as_ref()
                    .or(col.informat.as_ref())
                    .map_or("", |f| &f.name),
                storage,
            );
        }
        at = at
            .checked_add(size)
            .ok_or_else(|| invalid("XPT label offset"))?;
    }
    if meta
        .label_lengths
        .iter()
        .enumerate()
        .any(|(i, &n)| n > 40 && !seen.contains(&i))
    {
        return Err(invalid("XPT missing long label entry"));
    }
    let pad = padding(at);
    if bytes(input, at, pad)?.iter().any(|&b| b != b' ') {
        return Err(invalid("XPT label padding"));
    }
    at.checked_add(pad)
        .ok_or_else(|| invalid("XPT label offset"))
}

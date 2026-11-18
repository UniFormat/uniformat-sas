use super::{bytes, decode_number, invalid, Metadata};
use crate::{ColumnKind, NumValue, Result};
use std::borrow::Cow;

/// Borrowed observation with validated column boundaries.
#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    data: &'a [u8],
    meta: &'a Metadata,
}
impl<'a> Row<'a> {
    /// Reject incomplete/oversized observations before exposing fields.
    pub fn new(data: &'a [u8], meta: &'a Metadata) -> Result<Self> {
        if data.len() != meta.row_length() as usize {
            return Err(invalid("XPT observation width"));
        }
        Ok(Self { data, meta })
    }
    /// Raw field bytes, including trailing spaces.
    pub fn bytes(&self, column: usize) -> Result<&'a [u8]> {
        let c = self
            .meta
            .columns()
            .get(column)
            .ok_or_else(|| invalid("XPT column index"))?;
        bytes(self.data, c.offset as usize, c.width())
    }
    /// IBM number or tagged missing; rejects character columns.
    pub fn number(&self, column: usize) -> Result<NumValue> {
        if !self
            .meta
            .columns()
            .get(column)
            .is_some_and(|c| matches!(c.kind, ColumnKind::Numeric { .. }))
        {
            return Err(invalid("XPT non-numeric column"));
        }
        decode_number(self.bytes(column)?)
    }
    /// Character value using the explicitly selected encoding.
    pub fn text(&self, column: usize, trim_trailing: bool) -> Result<Cow<'a, str>> {
        if !self
            .meta
            .columns()
            .get(column)
            .is_some_and(|c| matches!(c.kind, ColumnKind::Char { .. }))
        {
            return Err(invalid("XPT non-character column"));
        }
        let data = self.bytes(column)?;
        let end = if trim_trailing {
            data.iter()
                .rposition(|&b| b != b' ')
                .map_or(0, |n| n.saturating_add(1))
        } else {
            data.len()
        };
        self.meta.encoding().decode(bytes(data, 0, end)?)
    }
}

//! Container-specific cell access; no synthetic SAS pages or on-disk recoding.
use crate::Result;
use uniformat_sas::{Column, NumValue, RowRef, TextPolicy, TextValue};

pub(crate) trait ColumnRow {
    fn physical_index(&self) -> u64;
    fn number(&self, column: &Column) -> Result<NumValue>;
    fn text_value(&self, column: &Column, policy: &TextPolicy) -> Result<TextValue<'_>>;
    fn bytes(&self, column: &Column, trim: bool) -> Result<&[u8]>;
}
impl ColumnRow for RowRef<'_> {
    fn physical_index(&self) -> u64 {
        RowRef::physical_index(self)
    }
    fn number(&self, c: &Column) -> Result<NumValue> {
        Ok(self.num(c))
    }
    fn text_value(&self, c: &Column, p: &TextPolicy) -> Result<TextValue<'_>> {
        Ok(RowRef::text_value(self, c, p)?)
    }
    fn bytes(&self, c: &Column, trim: bool) -> Result<&[u8]> {
        Ok(if trim {
            self.char_bytes(c)
        } else {
            RowRef::bytes(self, c)
        })
    }
}

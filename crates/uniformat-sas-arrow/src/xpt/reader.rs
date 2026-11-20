use super::{MemberSelection, XptReadOptions};
use crate::column_builder::{Builder, OutColumn};
use crate::row_values::ColumnRow;
use crate::{Error, ReadStats, Result, TrailingBlanks};
use arrow_array::{RecordBatch, RecordBatchOptions, RecordBatchReader};
use arrow_schema::{ArrowError, SchemaRef};
use std::sync::Arc;
use uniformat_sas::xpt::Row;
use uniformat_sas::{Column, NumValue, TextPolicy, TextValue};
use uniformat_sas_io::{xpt::Member, ByteSource, XptFile};

struct TransportRow<'a> {
    row: Row<'a>,
    index: u64,
}
impl ColumnRow for TransportRow<'_> {
    fn physical_index(&self) -> u64 {
        self.index
    }
    fn number(&self, c: &Column) -> Result<NumValue> {
        Ok(self.row.number(c.index)?)
    }
    fn text_value(&self, c: &Column, p: &TextPolicy) -> Result<TextValue<'_>> {
        Ok(TextValue::Text {
            value: self.row.text(c.index, p.trim_trailing)?,
            replaced: false,
        })
    }
    fn bytes(&self, c: &Column, trim: bool) -> Result<&[u8]> {
        let b = self.row.bytes(c.index)?;
        let end = if trim {
            b.iter().rposition(|&v| v != b' ').map_or(0, |i| i + 1)
        } else {
            b.len()
        };
        Ok(&b[..end])
    }
}
/// Bounded, fused Arrow stream for a selected XPORT member.
pub struct XptBatchReader<S: ByteSource> {
    file: XptFile<S>,
    member: Member,
    schema: SchemaRef,
    columns: Vec<OutColumn>,
    options: XptReadOptions,
    next: u64,
    end: u64,
    batch_rows: usize,
    failed: bool,
    stats: ReadStats,
}
impl<S: ByteSource> XptBatchReader<S> {
    /// Index members and construct the stream. For a metadata-only read use `xpt_schema`.
    pub fn new(
        file: XptFile<S>,
        options: XptReadOptions,
        projection: Option<Vec<usize>>,
        limit: Option<u64>,
    ) -> Result<Self> {
        let member = select(&file, &options.member)?;
        Self::with_member(file, member, options, projection, limit)
    }
    pub(super) fn with_member(
        file: XptFile<S>,
        member: Member,
        options: XptReadOptions,
        projection: Option<Vec<usize>>,
        limit: Option<u64>,
    ) -> Result<Self> {
        let plans = super::schema::plans(member.metadata(), &options, projection.as_deref())?;
        let schema = super::schema::from_plans(member.metadata(), &plans);
        let budget = crate::batch_budget::column_max_rows(
            member.metadata().columns(),
            &plans,
            &options.read,
        )?;
        let raw_budget = options.read.max_batch_bytes / 4 / member.metadata().row_length() as usize;
        let batch_rows = options
            .read
            .batch_rows
            .min(budget)
            .min(raw_budget)
            .min(1 << 20);
        if batch_rows == 0 {
            return Err(Error::ResourceLimit(
                "XPT batch budget too small for one observation",
            ));
        }
        let end = member.row_count().min(limit.unwrap_or(u64::MAX));
        let capacity = batch_rows
            .min(1024)
            .min(usize::try_from(end).unwrap_or(usize::MAX));
        let columns = plans
            .into_iter()
            .map(|p| OutColumn {
                source: p.source,
                builder: Builder::new(p.repr, capacity),
            })
            .collect();
        Ok(Self {
            file,
            member,
            schema,
            columns,
            options,
            next: 0,
            end,
            batch_rows,
            failed: false,
            stats: Default::default(),
        })
    }
    /// Schema including projection and sidecars.
    pub fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
    /// Conversion counters.
    pub fn stats(&self) -> &ReadStats {
        &self.stats
    }
    fn batch(&mut self) -> Result<RecordBatch> {
        let count = (self.end - self.next).min(self.batch_rows as u64) as usize;
        let width = self.member.metadata().row_length() as usize;
        let offset = self
            .next
            .checked_mul(width as u64)
            .and_then(|n| self.member.data_offset().checked_add(n))
            .ok_or(Error::ResourceLimit("XPT observation offset overflow"))?;
        let length = count
            .checked_mul(width)
            .ok_or(Error::ResourceLimit("XPT batch size overflow"))?;
        let bytes = self
            .file
            .source()
            .read_range(offset, length)
            .map_err(uniformat_sas_io::IoError::from)?;
        let policy = TextPolicy {
            invalid: self.options.read.invalid_bytes,
            trim_trailing: self.options.read.trailing_blanks == TrailingBlanks::Trim,
        };
        for (i, data) in bytes.chunks_exact(width).enumerate() {
            let row = TransportRow {
                row: Row::new(data, self.member.metadata())?,
                index: self.next + i as u64,
            };
            let mut error = None;
            for c in &mut self.columns {
                let col = &self.member.metadata().columns()[c.source];
                crate::column_append::append(
                    c,
                    col,
                    &row,
                    &self.options.read,
                    &policy,
                    &mut self.stats,
                    &mut error,
                );
                if let Some(e) = error {
                    return Err(e);
                }
            }
            self.stats.rows += 1;
        }
        self.next += count as u64;
        Ok(RecordBatch::try_new_with_options(
            self.schema(),
            self.columns
                .iter_mut()
                .map(|c| c.builder.finish())
                .collect(),
            &RecordBatchOptions::new().with_row_count(Some(count)),
        )?)
    }
}
impl<S: ByteSource> Iterator for XptBatchReader<S> {
    type Item = std::result::Result<RecordBatch, ArrowError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.next >= self.end {
            return None;
        }
        let result = self.batch();
        self.failed = result.is_err();
        Some(result.map_err(|e| ArrowError::ExternalError(Box::new(e))))
    }
}
impl<S: ByteSource> std::iter::FusedIterator for XptBatchReader<S> {}
impl<S: ByteSource> RecordBatchReader for XptBatchReader<S> {
    fn schema(&self) -> SchemaRef {
        self.schema()
    }
}

pub(super) fn select<S: ByteSource>(
    file: &XptFile<S>,
    selection: &MemberSelection,
) -> Result<Member> {
    let mut members = file.members()?;
    let index = match selection {
        MemberSelection::First => 0,
        MemberSelection::Index(i) => *i,
        MemberSelection::Name(name) => {
            let matches = members
                .iter()
                .enumerate()
                .filter(|(_, m)| m.metadata().header().name.eq_ignore_ascii_case(name))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(Error::Unsupported("XPT member name absent or ambiguous"));
            }
            matches[0]
        }
    };
    if index >= members.len() {
        return Err(Error::Unsupported("XPT member index out of range"));
    }
    let member = members.swap_remove(index);
    Ok(member)
}

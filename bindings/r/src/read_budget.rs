//! Conservative output limits for R imports; file metadata must never dictate an allocation.
use crate::convert::{RData, RError, RReadOptions};
use uniformat_sas::{Column, ColumnKind};

pub(crate) struct ReadBudget {
    max_rows: usize,
}

fn exceeded() -> RError {
    RError::ResourceLimit("budget mémoire de lecture dépassé ; réduire n_max/col_select ou augmenter max_bytes pour une source fiable".to_owned())
}

impl ReadBudget {
    pub(crate) fn new(
        columns: &[&Column],
        declared: u64,
        opts: &RReadOptions,
    ) -> Result<Self, RError> {
        // Include output descriptors, vector growth and the worst UTF-8 expansion (three bytes
        // per input byte). This is a bound for conversion buffers, not the entire R process RSS.
        let overhead = columns.len().checked_mul(256).ok_or_else(exceeded)?;
        let available = opts.max_bytes.checked_sub(overhead).ok_or_else(exceeded)?;
        let row_bytes = columns.iter().try_fold(1usize, |sum, col| {
            let size = match col.kind {
                ColumnKind::Numeric { .. } => std::mem::size_of::<f64>(),
                ColumnKind::Char { .. } => col
                    .width()
                    .checked_mul(3)
                    .and_then(|n| n.checked_add(std::mem::size_of::<Option<String>>()))
                    .ok_or_else(exceeded)?,
            };
            sum.checked_add(size).ok_or_else(exceeded)
        })?;
        let max_rows = (available / row_bytes / 2).min(i32::MAX as usize);
        let requested = opts.n_max.map_or(declared, |n| declared.min(n as u64));
        if requested > max_rows as u64 {
            return Err(exceeded());
        }
        Ok(Self { max_rows })
    }

    pub(crate) fn reserve_row(&self, rows: usize, data: &mut [RData]) -> Result<(), RError> {
        // Check actual rows too: the file may understate its row count.
        if rows >= self.max_rows {
            return Err(exceeded());
        }
        for column in data {
            let result = match column {
                RData::Real(v) => v.try_reserve(1),
                RData::Str(v) => v.try_reserve(1),
            };
            result.map_err(|_| exceeded())?;
        }
        Ok(())
    }
}

//! Estimate output storage before allocating, including UTF-8 expansion and buffer growth.
use crate::error::{Error, Result};
use crate::options::ReadOptions;
use crate::schema::{FieldPlan, Repr};
use uniformat_sas::TableMeta;

pub(crate) fn max_rows(
    meta: &TableMeta,
    fields: &[FieldPlan],
    opts: &ReadOptions,
) -> Result<usize> {
    let exceeded = || Error::ResourceLimit("budget mémoire du lot dépassé");
    // Per-column builder/alignment overhead; geometric growth and a transient decoded string
    // are included by the factor four. This bounds output buffers, not total process RSS.
    let overhead = fields.len().checked_mul(256).ok_or_else(exceeded)?;
    let available = opts
        .max_batch_bytes
        .checked_sub(overhead)
        .ok_or_else(exceeded)?;
    let row_bytes = fields.iter().try_fold(1usize, |sum, p| {
        let width = meta.columns[p.source].width();
        let data = match p.repr {
            Repr::Utf8 => width.checked_mul(3).and_then(|n| n.checked_add(8)),
            Repr::Binary => width.checked_add(8),
            Repr::Date32 => Some(4),
            Repr::SidecarU8 => Some(1),
            _ => Some(8),
        }
        .ok_or_else(exceeded)?;
        sum.checked_add(data.checked_add(1).ok_or_else(exceeded)?)
            .ok_or_else(exceeded)
    })?;
    let rows = available / row_bytes / 4;
    if rows == 0 {
        return Err(exceeded());
    }
    Ok(rows)
}

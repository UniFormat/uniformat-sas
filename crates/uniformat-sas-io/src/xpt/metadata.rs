use super::{file::invalid, XptOpenOptions};
use crate::{ByteSource, Result};
use uniformat_sas::xpt::{self, LabelHeader, Metadata, Version};

pub(super) fn metadata<S: ByteSource>(
    source: &S,
    offset: u64,
    opts: &XptOpenOptions,
    version: Version,
) -> Result<(Metadata, u64)> {
    let h = xpt::parse_member(
        &source.read_range(offset, 400)?,
        opts.encoding,
        &opts.limits,
    )?;
    if h.version != version {
        return Err(invalid("XPT member/library version mismatch"));
    }
    let size = h
        .variables
        .checked_mul(h.descriptor_size)
        .ok_or_else(|| invalid("XPT descriptor size"))?;
    let size = size
        .checked_add(xpt::padding(size))
        .filter(|&n| n <= opts.limits.max_text_bytes)
        .ok_or_else(|| invalid("XPT descriptor padding or budget"))?;
    let start = offset
        .checked_add(400)
        .ok_or_else(|| invalid("XPT metadata offset"))?;
    let mut at = start
        .checked_add(size as u64)
        .ok_or_else(|| invalid("XPT extension offset"))?;
    let meta = Metadata::parse(
        h,
        &source.read_range(start, size)?,
        opts.encoding,
        &opts.limits,
    )?;
    let mut extension = Vec::new();
    append(source, &mut at, &mut extension, 80, size, opts)?;
    if let Some(labels) = LabelHeader::parse(&extension, meta.columns().len())? {
        if version != Version::V8 {
            return Err(invalid("XPT labels require v8"));
        }
        for _ in 0..labels.count() {
            let entry_at = extension.len();
            append(
                source,
                &mut at,
                &mut extension,
                labels.prefix_len(),
                size,
                opts,
            )?;
            let prefix = extension
                .get(entry_at..)
                .ok_or_else(|| invalid("XPT label prefix"))?;
            let payload = labels
                .entry_len(prefix)?
                .checked_sub(labels.prefix_len())
                .ok_or_else(|| invalid("XPT label size"))?;
            append(source, &mut at, &mut extension, payload, size, opts)?;
        }
        let pad = xpt::padding(extension.len());
        append(source, &mut at, &mut extension, pad, size, opts)?;
        append(source, &mut at, &mut extension, 80, size, opts)?;
    }
    Ok((meta.complete(&extension, &opts.limits)?, at))
}

// Bound the total encoded metadata before every variable-size source read/allocation.
fn append<S: ByteSource>(
    source: &S,
    at: &mut u64,
    dst: &mut Vec<u8>,
    n: usize,
    descriptors: usize,
    opts: &XptOpenOptions,
) -> Result<()> {
    descriptors
        .checked_add(dst.len())
        .and_then(|v| v.checked_add(n))
        .filter(|&v| v <= opts.limits.max_text_bytes)
        .ok_or_else(|| invalid("XPT extension metadata budget"))?;
    let end = at
        .checked_add(n as u64)
        .ok_or_else(|| invalid("XPT extension offset"))?;
    dst.extend_from_slice(&source.read_range(*at, n)?);
    *at = end;
    Ok(())
}

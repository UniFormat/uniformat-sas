use super::{invalid, padding, put, write_schema::record, Version, WriteOptions};
use crate::{write::WriteSchema, Limits, Result, SasFormat};

fn long_format(f: &Option<SasFormat>) -> bool {
    f.as_ref().is_some_and(|f| f.name.len() > 8)
}
fn description(f: &Option<SasFormat>) -> String {
    match f {
        None => String::new(),
        Some(f) if f.width == 0 && f.decimals == 0 => format!("{}.", f.name),
        Some(f) if f.decimals == 0 => format!("{}{}.", f.name, f.width),
        Some(f) => format!("{}{}.{}", f.name, f.width, f.decimals),
    }
}
pub(super) fn extensions(
    schema: &WriteSchema,
    opts: &WriteOptions,
    limits: &Limits,
) -> Result<Vec<u8>> {
    if opts.version == Version::V5 {
        return Ok(Vec::new());
    }
    let extended = schema
        .columns
        .iter()
        .any(|c| long_format(&c.format) || long_format(&c.informat));
    let mut payload = Vec::new();
    let mut count = 0usize;
    for (i, col) in schema.columns.iter().enumerate() {
        let label = opts.encoding.encode(&col.label)?;
        if label.len() <= 40 && !long_format(&col.format) && !long_format(&col.informat) {
            continue;
        }
        let fmt = description(&col.format);
        let inf = description(&col.informat);
        let mut entry = Vec::new();
        let mut words = vec![i.saturating_add(1), col.name.len(), label.len()];
        if extended {
            words.extend([fmt.len(), inf.len()]);
        }
        for word in words {
            entry.extend_from_slice(
                &u16::try_from(word)
                    .map_err(|_| invalid("XPT label length"))?
                    .to_be_bytes(),
            );
        }
        entry.extend_from_slice(col.name.as_bytes());
        entry.extend_from_slice(&label);
        if extended {
            entry.extend_from_slice(fmt.as_bytes());
            entry.extend_from_slice(inf.as_bytes());
        }
        let size = payload
            .len()
            .checked_add(entry.len())
            .filter(|&n| n <= limits.max_text_bytes)
            .ok_or_else(|| invalid("XPT label budget"))?;
        payload.reserve(size.saturating_sub(payload.len()));
        payload.extend_from_slice(&entry);
        count = count.saturating_add(1);
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut out = record(if extended { "LABELV9" } else { "LABELV8" });
    put(&mut out, 48, &[b' '; 32])?;
    put(&mut out, 48, format!("{count:<5}").as_bytes())?;
    out.extend_from_slice(&payload);
    out.resize(out.len().saturating_add(padding(out.len())), b' ');
    Ok(out)
}

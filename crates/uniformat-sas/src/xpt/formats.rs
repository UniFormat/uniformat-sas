use super::{decimal, invalid};
use crate::{Result, SasFormat};

pub(super) fn parse_format(input: &[u8], base: &Option<SasFormat>) -> Result<Option<SasFormat>> {
    if input.is_empty() {
        if base.is_some() {
            return Err(invalid("XPT missing extended format"));
        }
        return Ok(None);
    }
    let text = std::str::from_utf8(input).map_err(|_| invalid("XPT format text"))?;
    let (stem, decimals) = match text.split_once('.') {
        Some((s, "")) => (s, 0),
        Some((s, d)) => (s, decimal(d.as_bytes())?),
        None => (text, 0),
    };
    let name_len = stem.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let name = stem
        .get(..name_len)
        .ok_or_else(|| invalid("XPT format name"))?;
    let width = if name_len == stem.len() {
        0
    } else {
        decimal(
            stem.get(name_len..)
                .ok_or_else(|| invalid("XPT format width"))?
                .as_bytes(),
        )?
    };
    if name.len() > 32
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
        || width > i16::MAX as usize
        || decimals > i16::MAX as usize
        || base
            .as_ref()
            .map_or((0, 0), |f| (usize::from(f.width), usize::from(f.decimals)))
            != (width, decimals)
    {
        return Err(invalid("XPT inconsistent extended format"));
    }
    if base.as_ref().is_none_or(|f| {
        name.get(..f.name.len())
            .is_none_or(|prefix| !prefix.eq_ignore_ascii_case(&f.name))
    }) {
        return Err(invalid("XPT inconsistent extended format name"));
    }
    Ok(Some(SasFormat {
        name: name.into(),
        width: width as u16,
        decimals: decimals as u16,
    }))
}

use super::write_schema::{descriptors, field, name, record};
use super::{invalid, put, Metadata, TextEncoding, Version};
use crate::write::{CellValue, WriteSchema};
use crate::{Limits, MissingKind, NumValue, Result};

/// Deterministic, strict transport write options. No implicit system clock.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Explicit transport version (v5 by default).
    pub version: Version,
    /// Promised observation count, v8 only; checked by the streaming writer.
    pub row_count: Option<u64>,
    /// Dataset name: 1–8 ASCII bytes in v5, 1–32 in v8.
    pub table_name: String,
    /// Dataset label, at most 40 encoded bytes.
    pub table_label: String,
    /// Explicit text encoding (not stored in XPORT).
    pub encoding: TextEncoding,
    /// Creation date, exactly ddMMMyy:hh:mm:ss (1969–2068 pivot).
    pub created: String,
    /// Modification date, same format as created.
    pub modified: String,
}
impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            version: Version::V5,
            row_count: None,
            table_name: "DATASET".into(),
            table_label: String::new(),
            encoding: TextEncoding::Utf8,
            created: "01JAN00:00:00:00".into(),
            modified: "01JAN00:00:00:00".into(),
        }
    }
}

/// Validated buffer encoder. Validates the complete schema before producing any bytes.
#[derive(Debug)]
pub struct Encoder {
    header: Vec<u8>,
    meta: Metadata,
    expected_rows: Option<u64>,
}
impl Encoder {
    /// Build library and member metadata, NAMESTR descriptors and OBS marker.
    pub fn new(schema: &WriteSchema, opts: &WriteOptions, limits: &Limits) -> Result<Self> {
        name(&opts.table_name, opts.version)?;
        if opts
            .row_count
            .is_some_and(|n| opts.version != Version::V8 || n > 999_999_999_999_999)
        {
            return Err(invalid(
                "XPT row_count requires v8 and at most 15 decimal digits",
            ));
        }
        super::date::validate_date(opts.created.as_bytes())?;
        super::date::validate_date(opts.modified.as_bytes())?;
        let descriptors = descriptors(schema, opts, limits)?;
        let mut header = record(opts.version.marker("LIBRARY", "LIBV8"));
        let mut real = [b' '; 80];
        put(&mut real, 0, b"SAS     SAS     SASLIB  ")?;
        field(&mut real, 24, 8, "6.06", opts.encoding)?;
        field(&mut real, 32, 8, "UNIFORM", opts.encoding)?;
        put(&mut real, 64, opts.created.as_bytes())?;
        header.extend_from_slice(&real);
        let mut modified = [b' '; 80];
        put(&mut modified, 0, opts.modified.as_bytes())?;
        header.extend_from_slice(&modified);
        let mut member = record(opts.version.marker("MEMBER", "MEMBV8"));
        put(&mut member, 63, b"00160")?;
        put(&mut member, 75, b"140")?;
        header.extend_from_slice(&member);
        header.extend_from_slice(&record(opts.version.marker("DSCRPTR", "DSCPTV8")));
        real = [b' '; 80];
        put(&mut real, 0, b"SAS     ")?;
        field(
            &mut real,
            8,
            opts.version.name_len(),
            &opts.table_name,
            opts.encoding,
        )?;
        let ident = if opts.version == Version::V5 {
            16usize
        } else {
            40usize
        };
        put(&mut real, ident, b"SASDATA ")?;
        field(&mut real, ident.saturating_add(8), 8, "6.06", opts.encoding)?;
        field(
            &mut real,
            ident.saturating_add(16),
            8,
            "UNIFORM",
            opts.encoding,
        )?;
        put(&mut real, 64, opts.created.as_bytes())?;
        header.extend_from_slice(&real);
        field(&mut modified, 32, 40, &opts.table_label, opts.encoding)?;
        field(&mut modified, 72, 8, "DATA", opts.encoding)?;
        header.extend_from_slice(&modified);
        let mut namestr = record(opts.version.marker("NAMESTR", "NAMSTV8"));
        put(
            &mut namestr,
            53,
            format!("{:05}", schema.columns.len()).as_bytes(),
        )?;
        header.extend_from_slice(&namestr);
        let member = super::parse_member(super::bytes(&header, 240, 400)?, opts.encoding, limits)?;
        let mut extension = super::write_labels::extensions(schema, opts, limits)?;
        let mut obs = record(opts.version.marker("OBS", "OBSV8"));
        if let Some(n) = opts.row_count {
            put(&mut obs, 48, &[b' '; 32])?;
            put(&mut obs, 48, format!("{n:15}").as_bytes())?;
        }
        extension.extend_from_slice(&obs);
        let meta = Metadata::parse(member, &descriptors, opts.encoding, limits)?
            .complete(&extension, limits)?;
        header.extend_from_slice(&descriptors);
        header.extend_from_slice(&extension);
        Ok(Self {
            header,
            meta,
            expected_rows: opts.row_count,
        })
    }
    /// Observation count promised by the caller, including zero.
    pub fn expected_rows(&self) -> Option<u64> {
        self.expected_rows
    }
    /// Complete header through the OBS marker (aligned to 80 bytes).
    pub fn header(&self) -> &[u8] {
        &self.header
    }
    /// Validated metadata.
    pub fn metadata(&self) -> &Metadata {
        &self.meta
    }
    /// Encode one complete observation, or return an error without partial output.
    pub fn encode_row(&self, cells: &[CellValue<'_>]) -> Result<Vec<u8>> {
        if cells.len() != self.meta.columns().len() {
            return Err(invalid("XPT row cell count"));
        }
        let mut out = vec![b' '; self.meta.row_length() as usize];
        for (c, cell) in self.meta.columns().iter().zip(cells) {
            let offset = c.offset as usize;
            match c.kind {
                crate::ColumnKind::Numeric { length } => {
                    let v = match cell {
                        CellValue::Num(v) => NumValue::Value(*v),
                        CellValue::Missing(m) => NumValue::Missing(*m),
                        CellValue::Null => NumValue::Missing(MissingKind::Dot),
                        _ => return Err(invalid("XPT numeric cell type")),
                    };
                    let encoded = super::encode_number(v)?;
                    let n = usize::from(length);
                    if encoded.get(n..).is_some_and(|b| b.iter().any(|&v| v != 0)) {
                        return Err(invalid("XPT narrow numeric would lose precision"));
                    }
                    put(&mut out, offset, super::bytes(&encoded, 0, n)?)?;
                }
                crate::ColumnKind::Char { length } => {
                    let encoded = match cell {
                        CellValue::Text(s) => self.meta.encoding().encode(s)?,
                        CellValue::Bytes(b) => std::borrow::Cow::Borrowed(*b),
                        CellValue::Null => std::borrow::Cow::Borrowed(&[][..]),
                        _ => return Err(invalid("XPT character cell type")),
                    };
                    if encoded.len() > length as usize {
                        return Err(invalid("XPT text exceeds column width"));
                    }
                    put(&mut out, offset, &encoded)?;
                }
            }
        }
        Ok(out)
    }
}

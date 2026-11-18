use super::write_schema::{descriptors, field, name, record};
use super::{invalid, put, Metadata, TextEncoding};
use crate::write::{CellValue, WriteSchema};
use crate::{Limits, MissingKind, NumValue, Result};

/// Deterministic, strict v5 write options. No implicit system clock.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Dataset name, 1–8 ASCII identifier bytes.
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
}
impl Encoder {
    /// Build library and member metadata, NAMESTR descriptors and OBS marker.
    pub fn new(schema: &WriteSchema, opts: &WriteOptions, limits: &Limits) -> Result<Self> {
        name(&opts.table_name)?;
        super::date::validate_date(opts.created.as_bytes())?;
        super::date::validate_date(opts.modified.as_bytes())?;
        let descriptors = descriptors(schema, opts, limits)?;
        let mut header = record("LIBRARY");
        let mut real = [b' '; 80];
        put(&mut real, 0, b"SAS     SAS     SASLIB  ")?;
        field(&mut real, 24, 8, "6.06", opts.encoding)?;
        field(&mut real, 32, 8, "UNIFORM", opts.encoding)?;
        put(&mut real, 64, opts.created.as_bytes())?;
        header.extend_from_slice(&real);
        let mut modified = [b' '; 80];
        put(&mut modified, 0, opts.modified.as_bytes())?;
        header.extend_from_slice(&modified);
        let mut member = record("MEMBER");
        put(&mut member, 63, b"00160")?;
        put(&mut member, 75, b"140")?;
        header.extend_from_slice(&member);
        header.extend_from_slice(&record("DSCRPTR"));
        put(&mut real, 8, &[b' '; 8])?;
        field(&mut real, 8, 8, &opts.table_name, opts.encoding)?;
        put(&mut real, 16, b"SASDATA ")?;
        header.extend_from_slice(&real);
        field(&mut modified, 32, 40, &opts.table_label, opts.encoding)?;
        field(&mut modified, 72, 8, "DATA", opts.encoding)?;
        header.extend_from_slice(&modified);
        let mut namestr = record("NAMESTR");
        put(
            &mut namestr,
            54,
            format!("{:04}", schema.columns.len()).as_bytes(),
        )?;
        header.extend_from_slice(&namestr);
        let member = super::parse_member(super::bytes(&header, 240, 400)?, opts.encoding, limits)?;
        let meta = Metadata::parse(member, &descriptors, opts.encoding, limits)?;
        header.extend_from_slice(&descriptors);
        header.extend_from_slice(&record("OBS"));
        Ok(Self { header, meta })
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

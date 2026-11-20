use super::{MemberHeader, TextEncoding, Version};
use crate::{Column, Limits, Result};

/// Immutable validated metadata, safe to use for row slicing.
#[derive(Debug, Clone)]
pub struct Metadata {
    header: MemberHeader,
    pub(super) columns: Vec<Column>,
    row_length: u32,
    encoding: TextEncoding,
    pub(super) label_lengths: Vec<u16>,
    declared_rows: Option<u64>,
    metadata_bytes: usize,
}

impl Metadata {
    /// Validate NAMESTR descriptors; call `complete` for v8 extensions and OBS.
    pub fn parse(
        header: MemberHeader,
        descriptors: &[u8],
        encoding: TextEncoding,
        limits: &Limits,
    ) -> Result<Self> {
        let (columns, row_length) = super::namestr::parse_version(
            descriptors,
            header.variables,
            header.descriptor_size,
            encoding,
            limits,
            header.version,
        )?;
        let label_lengths = if header.version == Version::V8 {
            descriptors
                .chunks_exact(header.descriptor_size)
                .take(header.variables)
                .map(|d| super::u16_at(d, 120))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        Ok(Self {
            header,
            columns,
            row_length,
            encoding,
            label_lengths,
            declared_rows: None,
            metadata_bytes: descriptors.len(),
        })
    }
    /// Validate optional LABELV8/LABELV9 records and the observation header.
    /// Input ends immediately after OBS/OBSV8; no observations are consumed.
    pub fn complete(mut self, extension: &[u8], limits: &Limits) -> Result<Self> {
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(extension.len())
            .filter(|&n| n <= limits.max_text_bytes)
            .ok_or_else(|| super::invalid("XPT metadata budget"))?;
        let offset = super::labels::apply(&mut self, extension)?;
        let obs = extension
            .get(offset..)
            .ok_or_else(|| super::invalid("XPT OBS offset"))?;
        self.declared_rows = super::header::observation_header(obs, self.header.version)?;
        Ok(self)
    }
    /// Positive observation count declared by OBSV8, absent for legacy/zero counts.
    pub fn declared_row_count(&self) -> Option<u64> {
        self.declared_rows
    }
    /// Encoded metadata size retained for cumulative resource accounting.
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    /// Dataset metadata.
    pub fn header(&self) -> &MemberHeader {
        &self.header
    }
    /// Validated columns.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Observation width in bytes.
    pub fn row_length(&self) -> u32 {
        self.row_length
    }
    /// Explicit text encoding.
    pub fn encoding(&self) -> TextEncoding {
        self.encoding
    }
}

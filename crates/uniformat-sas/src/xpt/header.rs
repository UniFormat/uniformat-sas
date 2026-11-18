use super::date::validate_date;
use super::{bytes, decimal, invalid, TextEncoding};
use crate::{Column, Limits, Result};

/// Transport version detected from the library marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// XPORT version 5/6 layout.
    V5,
    /// Version 8 (recognized, not yet supported).
    V8,
}

/// Check the first physical record without reading the data section.
pub fn detect_version(input: &[u8]) -> Result<Version> {
    let record = bytes(input, 0, 80)?;
    if check_record(record, "LIBRARY").is_ok() {
        return Ok(Version::V5);
    }
    if check_record(record, "LIBV8").is_ok() {
        return Ok(Version::V8);
    }
    Err(invalid("XPT LIBRARY marker (CPORT is not XPORT)"))
}

/// Validate the three library records. LIBV8 returns a typed unsupported error.
pub fn parse_library(input: &[u8]) -> Result<Version> {
    let version = detect_version(input)?;
    if version == Version::V8 {
        return Err(crate::Error::UnsupportedFormat {
            what: "XPT v8 (LIBV8)",
        });
    }
    let records = bytes(input, 0, 240)?;
    if bytes(records, 80, 24)? != b"SAS     SAS     SASLIB  " {
        return Err(invalid("XPT library identifiers"));
    }
    validate_date(bytes(records, 144, 16)?)?;
    validate_date(bytes(records, 160, 16)?)?;
    Ok(version)
}

pub(super) fn check_record(input: &[u8], name: &str) -> Result<()> {
    let record = bytes(input, 0, 80)?;
    if bytes(record, 0, 20)? != b"HEADER RECORD*******"
        || bytes(record, 20, 8)? != format!("{name:<8}").as_bytes()
        || bytes(record, 28, 20)? != b"HEADER RECORD!!!!!!!"
        || !bytes(record, 48, 30)?.iter().all(u8::is_ascii_digit)
        || bytes(record, 78, 2)? != b"  "
    {
        return Err(invalid("XPT header record"));
    }
    Ok(())
}

/// True only for a complete MEMBER + DSCRPTR pair at a physical record boundary.
#[must_use]
pub fn is_member_boundary(input: &[u8]) -> bool {
    check_record(input, "MEMBER").is_ok()
        && bytes(input, 80, 80).is_ok_and(|b| check_record(b, "DSCRPTR").is_ok())
}

/// The five fixed metadata records preceding NAMESTR descriptors.
#[derive(Debug, Clone)]
pub struct MemberHeader {
    /// Dataset name.
    pub name: String,
    /// Dataset label.
    pub label: String,
    /// Dataset type (often empty or DATA).
    pub dataset_type: String,
    /// Creation date, as recorded (two-digit year; pivot 1969/2068).
    pub created: String,
    /// Modification date, as recorded.
    pub modified: String,
    /// Number of descriptors, checked against Limits.
    pub variables: usize,
    /// Descriptor size (135 or 140).
    pub descriptor_size: usize,
}

/// Parse MEMBER, DSCRPTR, member data 1/2 and NAMESTR. No observation bytes needed.
pub fn parse_member(input: &[u8], enc: TextEncoding, limits: &Limits) -> Result<MemberHeader> {
    check_record(input, "MEMBER")?;
    check_record(bytes(input, 80, 80)?, "DSCRPTR")?;
    check_record(bytes(input, 320, 80)?, "NAMESTR")?;
    if bytes(input, 63, 5)? != b"00160" {
        return Err(invalid("XPT member descriptor header size"));
    }
    let descriptor_size = decimal(bytes(input, 75, 3)?)?;
    let variables = decimal(bytes(input, 374, 4)?)?;
    if ![135, 140].contains(&descriptor_size)
        || variables == 0
        || variables > limits.max_columns as usize
        || variables
            .checked_mul(descriptor_size)
            .is_none_or(|n| n > limits.max_text_bytes)
        || bytes(input, 368, 6)? != b"000000"
    {
        return Err(invalid("XPT NAMESTR count, size or budget"));
    }
    if bytes(input, 160, 8)? != b"SAS     " || bytes(input, 176, 8)? != b"SASDATA " {
        return Err(invalid("XPT member identifiers"));
    }
    validate_date(bytes(input, 224, 16)?)?;
    validate_date(bytes(input, 240, 16)?)?;
    Ok(MemberHeader {
        name: enc.field(bytes(input, 168, 8)?)?,
        label: enc.field(bytes(input, 272, 40)?)?,
        dataset_type: enc.field(bytes(input, 312, 8)?)?,
        created: enc.field(bytes(input, 224, 16)?)?,
        modified: enc.field(bytes(input, 240, 16)?)?,
        variables,
        descriptor_size,
    })
}

/// Immutable validated metadata, safe to use for row slicing.
#[derive(Debug, Clone)]
pub struct Metadata {
    header: MemberHeader,
    columns: Vec<Column>,
    row_length: u32,
    encoding: TextEncoding,
}

impl Metadata {
    /// Validate descriptors against the already parsed member header.
    pub fn parse(
        header: MemberHeader,
        descriptors: &[u8],
        encoding: TextEncoding,
        limits: &Limits,
    ) -> Result<Self> {
        let (columns, row_length) = super::parse_namestrs(
            descriptors,
            header.variables,
            header.descriptor_size,
            encoding,
            limits,
        )?;
        Ok(Self {
            header,
            columns,
            row_length,
            encoding,
        })
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

/// Check an OBS marker independently of the observation bytes.
pub fn parse_observation_header(input: &[u8]) -> Result<()> {
    check_record(input, "OBS")
}

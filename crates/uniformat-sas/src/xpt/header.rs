use super::date::validate_date;
use super::{bytes, decimal, invalid, TextEncoding, Version};
use crate::{Limits, Result};

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

/// Validate the three library records and detect the layout.
pub fn parse_library(input: &[u8]) -> Result<Version> {
    let version = detect_version(input)?;
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
    check_marker(record, name)?;
    if !bytes(record, 48, 30)?.iter().all(u8::is_ascii_digit) || bytes(record, 78, 2)? != b"  " {
        return Err(invalid("XPT header record"));
    }
    Ok(())
}

pub(super) fn check_marker(input: &[u8], name: &str) -> Result<()> {
    let record = bytes(input, 0, 80)?;
    if bytes(record, 0, 20)? != b"HEADER RECORD*******"
        || bytes(record, 20, 8)? != format!("{name:<8}").as_bytes()
        || bytes(record, 28, 20)? != b"HEADER RECORD!!!!!!!"
    {
        return Err(invalid("XPT header record"));
    }
    Ok(())
}

/// True only for a complete MEMBER + DSCRPTR pair at a physical record boundary.
#[must_use]
pub fn is_member_boundary(input: &[u8]) -> bool {
    [Version::V5, Version::V8].into_iter().any(|v| {
        check_record(input, v.marker("MEMBER", "MEMBV8")).is_ok()
            && bytes(input, 80, 80)
                .is_ok_and(|b| check_record(b, v.marker("DSCRPTR", "DSCPTV8")).is_ok())
    })
}

/// The five fixed metadata records preceding NAMESTR descriptors.
#[derive(Debug, Clone)]
pub struct MemberHeader {
    /// Transport version for this member.
    pub version: Version,
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
    let version = if check_record(input, "MEMBER").is_ok() {
        Version::V5
    } else {
        check_record(input, "MEMBV8")?;
        Version::V8
    };
    check_record(bytes(input, 80, 80)?, version.marker("DSCRPTR", "DSCPTV8"))?;
    check_record(bytes(input, 320, 80)?, version.marker("NAMESTR", "NAMSTV8"))?;
    if bytes(input, 63, 5)? != b"00160" {
        return Err(invalid("XPT member descriptor header size"));
    }
    let descriptor_size = decimal(bytes(input, 75, 3)?)?;
    let variables = decimal(bytes(input, 373, 5)?)?;
    if ![135, 140].contains(&descriptor_size)
        || (version == Version::V8 && descriptor_size != 140)
        || variables > version.columns()
        || variables == 0
        || variables > limits.max_columns as usize
        || variables
            .checked_mul(descriptor_size)
            .is_none_or(|n| n > limits.max_text_bytes)
        || bytes(input, 368, 5)? != b"00000"
    {
        return Err(invalid("XPT NAMESTR count, size or budget"));
    }
    if bytes(input, 160, 8)? != b"SAS     "
        || bytes(input, if version == Version::V5 { 176 } else { 200 }, 8)? != b"SASDATA "
    {
        return Err(invalid("XPT member identifiers"));
    }
    validate_date(bytes(input, 224, 16)?)?;
    validate_date(bytes(input, 240, 16)?)?;
    Ok(MemberHeader {
        version,
        name: enc.field(bytes(input, 168, version.name_len())?)?,
        label: enc.field(bytes(input, 272, 40)?)?,
        dataset_type: enc.field(bytes(input, 312, 8)?)?,
        created: enc.field(bytes(input, 224, 16)?)?,
        modified: enc.field(bytes(input, 240, 16)?)?,
        variables,
        descriptor_size,
    })
}

/// Check an OBS marker independently of the observation bytes.
pub fn parse_observation_header(input: &[u8]) -> Result<()> {
    check_record(input, "OBS")
}

pub(super) fn observation_header(input: &[u8], version: Version) -> Result<Option<u64>> {
    if input.len() != 80 {
        return Err(invalid("XPT observation header size"));
    }
    if version == Version::V5 {
        parse_observation_header(input)?;
        return Ok(None);
    }
    check_marker(input, "OBSV8")?;
    // haven 2.5.5 writes two right-aligned 15-byte integers; the second is zero.
    // The old 30-zero suffix also parses as an unknown count and a zero reserve.
    if !zero_reserved(bytes(input, 63, 15)?) || bytes(input, 78, 2)? != b"  " {
        return Err(invalid("XPT OBSV8 suffix"));
    }
    let count = std::str::from_utf8(bytes(input, 48, 15)?)
        .map_err(|_| invalid("XPT OBSV8 count"))?
        .trim_matches(' ');
    if count.is_empty() {
        return Ok(None);
    }
    if !count.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("XPT OBSV8 count"));
    }
    let n: u64 = count
        .parse()
        .map_err(|_| invalid("XPT OBSV8 count overflow"))?;
    Ok((n > 0).then_some(n))
}

pub(super) fn zero_reserved(input: &[u8]) -> bool {
    std::str::from_utf8(input).is_ok_and(|s| s.trim_matches(' ').bytes().all(|b| b == b'0'))
}

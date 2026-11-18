use super::XptRows;
use crate::{ByteSource, IoError, Result};
use uniformat_sas::xpt::{self, Metadata, TextEncoding};
use uniformat_sas::Limits;

pub(super) fn invalid(what: &'static str) -> IoError {
    uniformat_sas::Error::CorruptMetadata { what }.into()
}

/// Resource and encoding choices for transport input.
#[derive(Debug, Clone)]
pub struct XptOpenOptions {
    /// Codec allocation limits.
    pub limits: Limits,
    /// Explicit encoding; XPORT contains no encoding tag.
    pub encoding: TextEncoding,
    /// Limit on the number of members retained by `members()`.
    pub max_members: usize,
}
impl Default for XptOpenOptions {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            encoding: TextEncoding::Utf8,
            max_members: 1024,
        }
    }
}

/// One validated dataset and its data extent within a library.
#[derive(Debug, Clone)]
pub struct Member {
    meta: Metadata,
    data_offset: u64,
    row_count: u64,
}
impl Member {
    /// Dataset metadata.
    pub fn metadata(&self) -> &Metadata {
        &self.meta
    }
    /// Count inferred using the documented largest-padding policy.
    pub fn row_count(&self) -> u64 {
        self.row_count
    }
    /// First observation offset.
    pub fn data_offset(&self) -> u64 {
        self.data_offset
    }
}

/// Open library. Opening reads only the first member's metadata, never observations.
#[derive(Debug)]
pub struct XptFile<S: ByteSource> {
    source: S,
    options: XptOpenOptions,
    first: Metadata,
    first_data: u64,
}
impl<S: ByteSource> XptFile<S> {
    /// Validate library and first member headers. The source must contain whole 80-byte records.
    pub fn open(source: S, options: XptOpenOptions) -> Result<Self> {
        if !source.len().is_multiple_of(80) || options.max_members == 0 {
            return Err(invalid("XPT file length or max_members"));
        }
        xpt::parse_library(&source.read_range(0, 240)?)?;
        let (first, first_data) = metadata(&source, 240, &options)?;
        Ok(Self {
            source,
            options,
            first,
            first_data,
        })
    }
    /// First member's schema, available without scanning its observations.
    pub fn metadata(&self) -> &Metadata {
        &self.first
    }
    /// Underlying source (e.g. for read counters).
    pub fn source(&self) -> &S {
        &self.source
    }
    /// Index all members. Requires a sequential scan because XPORT has no global index.
    /// The retained metadata and number of members are bounded by the open options.
    pub fn members(&self) -> Result<Vec<Member>> {
        let mut out = Vec::new();
        let mut meta = self.first.clone();
        let mut start = self.first_data;
        let mut retained = 0usize;
        loop {
            if out.len() >= self.options.max_members {
                return Err(invalid("XPT member count budget"));
            }
            retained = retained
                .checked_add(meta.columns().len().saturating_mul(140))
                .filter(|&n| n <= self.options.limits.max_text_bytes)
                .ok_or_else(|| invalid("XPT cumulative metadata budget"))?;
            let end = self.find_member_end(start)?;
            let size = end
                .checked_sub(start)
                .ok_or_else(|| invalid("XPT data extent"))?;
            let tail_len = size.min(79) as usize;
            let tail = self.source.read_range(end - tail_len as u64, tail_len)?;
            let row_count = xpt::observation_count(size, meta.row_length(), &tail)?;
            out.push(Member {
                meta,
                data_offset: start,
                row_count,
            });
            if end == self.source.len() {
                return Ok(out);
            }
            (meta, start) = metadata(&self.source, end, &self.options)?;
        }
    }
    /// Stream encoded observations of an indexed member; at most one row allocated per item.
    pub fn rows<'a>(&'a self, member: &'a Member) -> XptRows<'a, S> {
        XptRows::new(&self.source, member)
    }
    fn find_member_end(&self, mut at: u64) -> Result<u64> {
        // 800 records plus one lookahead record: catches a marker across a read boundary.
        const CHUNK: u64 = 64_000;
        while at < self.source.len() {
            let size = (self.source.len() - at).min(CHUNK + 80) as usize;
            let chunk = self.source.read_range(at, size)?;
            for offset in (0..size.min(CHUNK as usize)).step_by(80) {
                if chunk.get(offset..).is_some_and(xpt::is_member_boundary) {
                    return at
                        .checked_add(offset as u64)
                        .ok_or_else(|| invalid("XPT data offset"));
                }
            }
            at = at
                .checked_add(CHUNK.min(self.source.len() - at))
                .ok_or_else(|| invalid("XPT scan offset"))?;
        }
        Ok(self.source.len())
    }
}

fn metadata<S: ByteSource>(
    source: &S,
    offset: u64,
    opts: &XptOpenOptions,
) -> Result<(Metadata, u64)> {
    let h = xpt::parse_member(
        &source.read_range(offset, 400)?,
        opts.encoding,
        &opts.limits,
    )?;
    let size = h
        .variables
        .checked_mul(h.descriptor_size)
        .ok_or_else(|| invalid("XPT descriptor size"))?;
    let size = size
        .checked_add(xpt::padding(size))
        .ok_or_else(|| invalid("XPT descriptor padding"))?;
    let start = offset
        .checked_add(400)
        .ok_or_else(|| invalid("XPT metadata offset"))?;
    let obs = start
        .checked_add(size as u64)
        .ok_or_else(|| invalid("XPT OBS offset"))?;
    let meta = Metadata::parse(
        h,
        &source.read_range(start, size)?,
        opts.encoding,
        &opts.limits,
    )?;
    xpt::parse_observation_header(&source.read_range(obs, 80)?)?;
    let data = obs
        .checked_add(80)
        .ok_or_else(|| invalid("XPT data offset"))?;
    Ok((meta, data))
}

use crate::{AtomicFile, IoError, Result};
use std::io::{self, Write};
use std::path::Path;
use uniformat_sas::write::{CellValue, WriteSchema};
use uniformat_sas::xpt::{Encoder, WriteOptions};
use uniformat_sas::Limits;

fn invalid(what: &'static str) -> IoError {
    io::Error::new(io::ErrorKind::InvalidInput, what).into()
}

/// Streaming transport writer. A failed write poisons the writer; commit then fails.
/// The file specialization removes its temporary file on error or drop.
#[derive(Debug)]
pub struct XptWriter<W: Write> {
    sink: W,
    encoder: Encoder,
    rows: u64,
    remainder: usize,
    last_blank: bool,
    failed: bool,
    marker_tail: Vec<u8>,
}
impl<W: Write> XptWriter<W> {
    /// Validate before touching `sink`, then write the header. No seek is required.
    pub fn new(
        sink: W,
        schema: &WriteSchema,
        opts: &WriteOptions,
        limits: &Limits,
    ) -> Result<Self> {
        let encoder = Encoder::new(schema, opts, limits)?;
        Self::from_encoder(sink, encoder)
    }
    fn from_encoder(mut sink: W, encoder: Encoder) -> Result<Self> {
        sink.write_all(encoder.header())?;
        Ok(Self {
            sink,
            encoder,
            rows: 0,
            remainder: 0,
            last_blank: false,
            failed: false,
            marker_tail: Vec::with_capacity(160),
        })
    }
    /// Successfully written observations.
    pub fn rows_written(&self) -> u64 {
        self.rows
    }
    /// Encode and write one row. Any error permanently disables commit.
    pub fn write_row(&mut self, cells: &[CellValue<'_>]) -> Result<()> {
        if self.failed {
            return Err(invalid("XPT writer failed; discard it"));
        }
        self.failed = true;
        let row = self.encoder.encode_row(cells)?;
        let rows = self
            .rows
            .checked_add(1)
            .ok_or_else(|| invalid("XPT row count overflow"))?;
        if self.encoder.expected_rows().is_some_and(|n| rows > n) {
            return Err(invalid("XPT row count exceeds promised count"));
        }
        if self.encoder.expected_rows().is_none() {
            self.check_marker_collision(&row)?;
        }
        self.sink.write_all(&row)?;
        self.remainder = (self.remainder + row.len() % 80) % 80;
        self.last_blank = row.iter().all(|&b| b == b' ');
        self.rows = rows;
        self.failed = false;
        Ok(())
    }
    // ARCHITECTURE NOTE: the container has no escaping. A literal MEMBER/DSCRPTR
    // pair in observations would be indexed as a new dataset. Reject it, including
    // pairs split over rows, instead of writing a file that loses data on re-read.
    fn check_marker_collision(&mut self, mut data: &[u8]) -> Result<()> {
        while !data.is_empty() {
            let n = (160 - self.marker_tail.len()).min(data.len());
            self.marker_tail.extend_from_slice(&data[..n]);
            data = &data[n..];
            if self.marker_tail.len() == 160 {
                if uniformat_sas::xpt::is_member_boundary(&self.marker_tail) {
                    return Err(invalid(
                        "XPT observation bytes collide with a member boundary",
                    ));
                }
                self.marker_tail.drain(..80);
            }
        }
        Ok(())
    }
    /// Pad and flush; reject an ambiguous all-blank final short observation.
    /// Returns the sink and number of observations.
    pub fn finish(mut self) -> Result<(W, u64)> {
        if self.failed {
            return Err(invalid("XPT writer failed; commit refused"));
        }
        let pad = (80 - self.remainder) % 80;
        let width = self.encoder.metadata().row_length() as usize;
        if self.encoder.expected_rows().is_some_and(|n| n != self.rows) {
            return Err(invalid("XPT row count differs from promised count"));
        }
        if self.encoder.expected_rows().is_none() && self.last_blank && width + pad < 80 {
            return Err(invalid(
                "XPT final blank observation is indistinguishable from padding",
            ));
        }
        if self.encoder.expected_rows().is_none() {
            self.check_marker_collision(&[b' '; 80][..pad])?;
        }
        self.sink.write_all(&[b' '; 80][..pad])?;
        self.sink.flush()?;
        Ok((self.sink, self.rows))
    }
}
impl XptWriter<AtomicFile> {
    /// Validate before creating a temporary file in the destination directory.
    pub fn create(
        path: impl AsRef<Path>,
        schema: &WriteSchema,
        opts: &WriteOptions,
        limits: &Limits,
    ) -> Result<Self> {
        let encoder = Encoder::new(schema, opts, limits)?;
        Self::from_encoder(AtomicFile::create(path)?, encoder)
    }
    /// Finish and atomically replace the destination. Failure/drop preserves the destination.
    pub fn commit(self) -> Result<u64> {
        let (sink, rows) = self.finish()?;
        sink.commit()?;
        Ok(rows)
    }
}

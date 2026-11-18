use super::XptWriteOptions;
use crate::{writer_rows::RowEncoder, ArrowWriteOptions, Error, Result};
use arrow_array::RecordBatch;
use arrow_schema::{ArrowError, SchemaRef};
use std::{io::Write, path::Path};
use uniformat_sas::{
    write::{WriteEncoding, WriteOptions},
    xpt::TextEncoding,
};
use uniformat_sas_io::{AtomicFile, XptWriter};

fn arrow_options(opts: &XptWriteOptions) -> ArrowWriteOptions {
    ArrowWriteOptions {
        write: WriteOptions {
            encoding: encoding(opts.write.encoding),
            ..WriteOptions::default()
        },
        missing_suffix: opts.missing_suffix.clone(),
        char_lengths: opts.char_lengths.clone(),
    }
}
fn encoding(enc: TextEncoding) -> WriteEncoding {
    match enc {
        TextEncoding::Utf8 => WriteEncoding::Utf8,
        TextEncoding::Latin1 => WriteEncoding::Latin1,
        TextEncoding::Windows1252 => WriteEncoding::Wlatin1,
    }
}
fn header(schema: &SchemaRef, opts: &XptWriteOptions) -> uniformat_sas::xpt::WriteOptions {
    let mut h = opts.write.clone();
    // Explicit options take precedence; defaults inherit transport metadata for round trips.
    if h.table_name == "DATASET" {
        if let Some(v) = schema.metadata().get("sas.table_name") {
            h.table_name.clone_from(v);
        }
    }
    if h.table_label.is_empty() {
        if let Some(v) = schema.metadata().get("sas.table_label") {
            h.table_label.clone_from(v);
        }
    }
    if h.created == "01JAN00:00:00:00" {
        if let Some(v) = schema.metadata().get("xpt.created") {
            h.created.clone_from(v);
        }
    }
    if h.modified == "01JAN00:00:00:00" {
        if let Some(v) = schema.metadata().get("xpt.modified") {
            h.modified.clone_from(v);
        }
    }
    h
}
/// Strict Arrow writer. Any failed batch permanently prevents finish/commit.
pub struct XptArrowWriter<W: Write> {
    inner: XptWriter<W>,
    encoder: RowEncoder,
    failed: bool,
}
impl<W: Write> XptArrowWriter<W> {
    /// Validate schema and write a deterministic transport header.
    pub fn try_new(sink: W, schema: SchemaRef, opts: XptWriteOptions) -> Result<Self> {
        let (ws, encoder) = RowEncoder::new(schema.clone(), &arrow_options(&opts), true)?;
        let inner = XptWriter::new(sink, &ws, &header(&schema, &opts), &opts.limits)?;
        Ok(Self {
            inner,
            encoder,
            failed: false,
        })
    }
    /// Write a batch with the schema supplied at construction.
    pub fn write(&mut self, batch: &RecordBatch) -> Result<()> {
        if self.failed {
            return Err(Error::Unsupported("XPT writer failed; discard it"));
        }
        self.failed = true;
        let inner = &mut self.inner;
        self.encoder.write(batch, |cells, _| {
            inner.write_row(cells)?;
            Ok(())
        })?;
        self.failed = false;
        Ok(())
    }
    /// Finalize padding and return the sink and number of observations.
    pub fn finish(self) -> Result<(W, u64)> {
        if self.failed {
            return Err(Error::Unsupported("XPT writer failed; finish refused"));
        }
        Ok(self.inner.finish()?)
    }
}
impl XptArrowWriter<AtomicFile> {
    /// Validate before creating a temporary file beside the destination.
    pub fn create(
        path: impl AsRef<Path>,
        schema: SchemaRef,
        opts: XptWriteOptions,
    ) -> Result<Self> {
        let (ws, encoder) = RowEncoder::new(schema.clone(), &arrow_options(&opts), true)?;
        let inner = XptWriter::create(path, &ws, &header(&schema, &opts), &opts.limits)?;
        Ok(Self {
            inner,
            encoder,
            failed: false,
        })
    }
    /// Atomically replace the destination after all conversions succeed.
    pub fn commit(self) -> Result<u64> {
        let (sink, rows) = self.finish()?;
        sink.commit().map_err(uniformat_sas_io::IoError::from)?;
        Ok(rows)
    }
}
/// Infer missing text widths within a memory budget, then write atomically.
/// Schemas already carrying widths are written as a bounded stream.
pub fn write_xpt_batches(
    path: impl AsRef<Path>,
    schema: SchemaRef,
    batches: impl IntoIterator<Item = std::result::Result<RecordBatch, ArrowError>>,
    mut opts: XptWriteOptions,
) -> Result<u64> {
    match RowEncoder::new(schema.clone(), &arrow_options(&opts), true) {
        Ok(_) => {
            let mut writer = XptArrowWriter::create(path, schema, opts)?;
            for batch in batches {
                writer.write(&batch?)?;
            }
            return writer.commit();
        }
        Err(Error::CharLengthUnknown(_)) => {}
        Err(e) => return Err(e),
    }
    let mut buffered = Vec::new();
    let mut bytes = 0usize;
    for batch in batches {
        let batch = batch?;
        // Include per-batch overhead, so even an endless stream of empty batches is bounded.
        bytes = bytes
            .checked_add(batch.get_array_memory_size().saturating_add(256))
            .ok_or(Error::ResourceLimit("XPT input buffer overflow"))?;
        if bytes > opts.max_buffer_bytes {
            return Err(Error::ResourceLimit(
                "XPT width inference exceeds max_buffer_bytes",
            ));
        }
        buffered.push(batch);
    }
    for (name, width) in crate::infer_char_lengths(&buffered, encoding(opts.write.encoding)) {
        opts.char_lengths.entry(name).or_insert(width);
    }
    // Empty tables still need a valid character descriptor.
    for f in schema.fields() {
        if matches!(
            f.data_type(),
            arrow_schema::DataType::Utf8
                | arrow_schema::DataType::LargeUtf8
                | arrow_schema::DataType::Utf8View
                | arrow_schema::DataType::Binary
                | arrow_schema::DataType::LargeBinary
        ) {
            opts.char_lengths.entry(f.name().clone()).or_insert(1);
        }
    }
    let mut writer = XptArrowWriter::create(path, schema, opts)?;
    for batch in buffered {
        writer.write(&batch)?;
    }
    writer.commit()
}

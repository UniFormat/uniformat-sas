//! The SAS `.sas7bdat` format as a `uniformat_core::Format`, so that the registry, the CLI and
//! `uniformat_core::convert` can read and write it through the common (Arrow) data model.
//!
//! Options are `key = value` strings (`ReadRequest::options`, `WriteRequest::options`):
//!
//! | read | values |
//! |---|---|
//! | `missing` | `null` (default), `sidecar` (adds `x__missing` columns) |
//! | `dates` | `native` (default), `float` (raw SAS values) |
//! | `trailing_blanks` | `trim` (default), `keep` |
//! | `timestamp_unit` | `us` (default), `s` |
//! | `time` | `time` (default), `duration` |
//! | `out_of_range` | `null` (default), `error` |
//! | `invalid_bytes` | `replace` (default), `error`, `binary` |
//! | `empty_as_null` | `true`, `false` (default) |
//! | `encoding` | label of an encoding that overrides the file header |
//! | `strict`, `batch_rows`, `plan` | `true`/`false`, a number, `fast`/`full` |
//!
//! | write | values |
//! |---|---|
//! | `compression` | `none` (default), `char`, `binary` |
//! | `encoding` | `utf-8` (default), `latin1`, `wlatin1` |
//! | `page_length`, `grow_page` | a number, `true`/`false` |
//! | `on_unrepresentable` | `error` (default), `replace`, `truncate` |
//! | `missing_suffix`, `table_name` | text |

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use uniformat_core::{
    BatchStream, Error, Format, ReadRequest, Registry, Result, TableInfo, WriteRequest,
    WriteSummary,
};
use uniformat_sas::write::{Unrepresentable, WriteEncoding};
use uniformat_sas::{Compression, Encoding, InvalidBytes, Strictness};
use uniformat_sas_io::{FileSource, PlanMode, SasFile};

use crate::{
    arrow_schema, infer_char_lengths, ArrowWriteOptions, ArrowWriter, BatchReader, DateMode,
    MissingMode, OutOfRange, ReadOptions, TimeMode, TimestampUnit, TrailingBlanks,
};

const NAME: &str = "sas7bdat";

/// The `.sas7bdat` format.
#[derive(Debug, Clone, Copy, Default)]
pub struct SasFormat;

/// Registers the SAS format in `registry`.
pub fn register(registry: &mut Registry) {
    registry.register(Arc::new(SasFormat));
}

fn convert_error(e: crate::Error) -> Error {
    use crate::Error as E;
    use uniformat_sas::write::WriteError;
    use uniformat_sas_io::{IoError, WriteIoError};
    match e {
        E::Io(IoError::Io(io)) | E::Write(WriteIoError::Io(io)) => Error::Io(io),
        E::Arrow(a) => Error::Arrow(a),
        E::Write(WriteIoError::Write(WriteError::Cell { .. })) => Error::Unrepresentable {
            format: NAME,
            message: e.to_string(),
        },
        E::Unsupported(what) => Error::Unsupported(what.to_owned()),
        other => Error::format(NAME, other),
    }
}

fn known(options: &BTreeMap<String, String>, allowed: &[&str]) -> Result<()> {
    for k in options.keys() {
        if !allowed.contains(&k.as_str()) {
            return Err(Error::option(k, "unknown option for the sas7bdat format"));
        }
    }
    Ok(())
}

fn parse_bool(key: &str, v: &str) -> Result<bool> {
    match v {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(Error::option(key, "expected true or false")),
    }
}

fn choice<T: Copy>(key: &str, v: &str, table: &[(&str, T)]) -> Result<T> {
    table
        .iter()
        .find(|(n, _)| *n == v)
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            let allowed: Vec<&str> = table.iter().map(|(n, _)| *n).collect();
            Error::option(key, format!("expected one of {}", allowed.join(", ")))
        })
}

/// Read options from `key = value` strings.
///
/// # Errors
/// Unknown key or invalid value.
pub fn read_options(options: &BTreeMap<String, String>) -> Result<ReadOptions> {
    known(
        options,
        &[
            "missing",
            "dates",
            "trailing_blanks",
            "timestamp_unit",
            "time",
            "out_of_range",
            "invalid_bytes",
            "empty_as_null",
            "encoding",
            "strict",
            "batch_rows",
            "max_batch_bytes",
            "max_output_columns",
            "plan",
        ],
    )?;
    let mut o = ReadOptions::default();
    for (k, v) in options {
        match k.as_str() {
            "missing" => {
                o.missing = choice(
                    k,
                    v,
                    &[
                        ("null", MissingMode::Null),
                        ("sidecar", MissingMode::Sidecar),
                    ],
                )?
            }
            "dates" => {
                o.dates = choice(
                    k,
                    v,
                    &[("native", DateMode::Native), ("float", DateMode::AsFloat)],
                )?
            }
            "trailing_blanks" => {
                o.trailing_blanks = choice(
                    k,
                    v,
                    &[
                        ("trim", TrailingBlanks::Trim),
                        ("keep", TrailingBlanks::Keep),
                    ],
                )?
            }
            "timestamp_unit" => {
                o.timestamp_unit = choice(
                    k,
                    v,
                    &[
                        ("us", TimestampUnit::Microsecond),
                        ("s", TimestampUnit::Second),
                    ],
                )?
            }
            "time" => {
                o.time_mode = choice(
                    k,
                    v,
                    &[("time", TimeMode::Time64), ("duration", TimeMode::Duration)],
                )?
            }
            "out_of_range" => {
                o.out_of_range = choice(
                    k,
                    v,
                    &[("null", OutOfRange::Null), ("error", OutOfRange::Error)],
                )?
            }
            "invalid_bytes" => {
                o.invalid_bytes = choice(
                    k,
                    v,
                    &[
                        ("replace", InvalidBytes::Replace),
                        ("error", InvalidBytes::Error),
                        ("binary", InvalidBytes::Binary),
                    ],
                )?
            }
            "empty_as_null" => o.empty_as_null = parse_bool(k, v)?,
            "encoding" => {
                o.encoding_override = Some(
                    Encoding::for_label(v.as_bytes())
                        .ok_or_else(|| Error::option(k, "unknown encoding"))?,
                );
            }
            "strict" => {
                o.strictness = if parse_bool(k, v)? {
                    Strictness::Strict
                } else {
                    Strictness::Lenient
                };
            }
            "batch_rows" | "max_batch_bytes" | "max_output_columns" => {
                let n: usize = v
                    .parse()
                    .map_err(|_| Error::option(k, "expected a positive number"))?;
                if n == 0 {
                    return Err(Error::option(k, "expected a positive number"));
                }
                match k.as_str() {
                    "batch_rows" => o.batch_rows = n,
                    "max_batch_bytes" => o.max_batch_bytes = n,
                    _ => o.max_output_columns = n,
                }
            }
            "plan" => o.plan = choice(k, v, &[("fast", PlanMode::Fast), ("full", PlanMode::Full)])?,
            _ => {}
        }
    }
    Ok(o)
}

/// Write options from `key = value` strings.
///
/// # Errors
/// Unknown key or invalid value.
pub fn write_options(options: &BTreeMap<String, String>) -> Result<ArrowWriteOptions> {
    known(
        options,
        &[
            "compression",
            "encoding",
            "page_length",
            "grow_page",
            "on_unrepresentable",
            "missing_suffix",
            "table_name",
        ],
    )?;
    let mut o = ArrowWriteOptions::default();
    for (k, v) in options {
        match k.as_str() {
            "compression" => {
                o.write.compression = choice(
                    k,
                    v,
                    &[
                        ("none", Compression::None),
                        ("char", Compression::Char),
                        ("binary", Compression::Binary),
                    ],
                )?
            }
            "encoding" => {
                o.write.encoding = choice(
                    k,
                    &v.to_ascii_lowercase(),
                    &[
                        ("utf-8", WriteEncoding::Utf8),
                        ("utf8", WriteEncoding::Utf8),
                        ("latin1", WriteEncoding::Latin1),
                        ("wlatin1", WriteEncoding::Wlatin1),
                        ("cp1252", WriteEncoding::Wlatin1),
                    ],
                )?;
            }
            "page_length" => {
                o.write.page_length = v
                    .parse()
                    .map_err(|_| Error::option(k, "expected a number"))?
            }
            "grow_page" => o.write.grow_page = parse_bool(k, v)?,
            "on_unrepresentable" => {
                o.write.on_unrepresentable = choice(
                    k,
                    v,
                    &[
                        ("error", Unrepresentable::Error),
                        ("replace", Unrepresentable::Replace),
                        ("truncate", Unrepresentable::Truncate),
                    ],
                )?
            }
            "missing_suffix" => o.missing_suffix.clone_from(v),
            "table_name" => o.write.table_name.clone_from(v),
            _ => {}
        }
    }
    Ok(o)
}

fn open(path: &Path, opts: &ReadOptions) -> Result<Arc<SasFile<FileSource>>> {
    let source = FileSource::open(path)?;
    let file = SasFile::open(source, opts.open_options()).map_err(|e| convert_error(e.into()))?;
    Ok(Arc::new(file))
}

fn projection(schema: &SchemaRef, columns: Option<&Vec<String>>) -> Result<Option<Vec<usize>>> {
    let Some(cols) = columns else { return Ok(None) };
    cols.iter()
        .map(|c| {
            schema
                .index_of(c)
                .map_err(|_| Error::option("columns", format!("column {c:?} not in the table")))
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

impl Format for SasFormat {
    fn name(&self) -> &'static str {
        NAME
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sas7bdat"]
    }

    fn can_write(&self) -> bool {
        true
    }

    fn info(&self, path: &Path, request: &ReadRequest) -> Result<TableInfo> {
        let opts = read_options(&request.options)?;
        let file = open(path, &opts)?;
        let schema = arrow_schema(file.meta(), &opts);
        let schema = match projection(&schema, request.columns.as_ref())? {
            Some(p) => Arc::new(schema.project(&p)?),
            None => schema,
        };
        Ok(TableInfo {
            schema,
            row_count: (file.meta().deleted_count == 0).then(|| file.meta().visible_rows()),
        })
    }

    fn read(&self, path: &Path, request: &ReadRequest) -> Result<BatchStream> {
        let opts = read_options(&request.options)?;
        let file = open(path, &opts)?;
        let schema = arrow_schema(file.meta(), &opts);
        let proj = projection(&schema, request.columns.as_ref())?;
        let pages = 0..file.meta().header.page_count;
        let reader =
            BatchReader::new(file, pages, proj, request.limit, opts).map_err(convert_error)?;
        Ok(Box::new(reader.into_arrow()))
    }

    fn write(
        &self,
        path: &Path,
        schema: SchemaRef,
        batches: BatchStream,
        request: &WriteRequest,
    ) -> Result<WriteSummary> {
        let mut opts = write_options(&request.options)?;
        // column lengths of text columns are measured on the data when the schema does not give them
        let batches: Vec<RecordBatch> = batches.collect::<std::result::Result<_, _>>()?;
        for (name, len) in infer_char_lengths(&batches, opts.write.encoding) {
            opts.char_lengths.entry(name).or_insert(len);
        }
        let mut w = ArrowWriter::create(path, schema, opts).map_err(convert_error)?;
        for b in &batches {
            w.write(b).map_err(convert_error)?;
        }
        let report = w.commit().map_err(convert_error)?;
        Ok(WriteSummary {
            rows: report.rows,
            warnings: report.warnings,
        })
    }
}

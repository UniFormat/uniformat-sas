//! Liaison Python (PyO3) : module natif `uniformat_io.sas._native` (ADR 0003 « Restitution Python », ADR 0007).
//!
//! - lecture : [`read_sas`] rend un flux Arrow (interface PyCapsule `__arrow_c_stream__`, sans copie vers pyarrow,
//!   polars ou pandas) ; [`read_table`] lit tout et émet les avertissements de lecture ;
//! - [`schema`] n'ouvre que l'en-tête et les pages de métadonnées ;
//! - erreurs : `SasError` (`SasFormatError`, `SasUnsupportedError`, `SasWriteError`), `OSError` pour les entrées/sorties ;
//! - une panique du cœur serait convertie par PyO3 en `pyo3_runtime.PanicException` : l'interpréteur ne tombe pas.
//!
//! Le code de ce crate ne contient aucun bloc `unsafe` ; seul le code généré par PyO3 en contient (ADR 0007 §2).
#![deny(unsafe_code)]

use std::ffi::CString;
use std::sync::Arc;

use arrow_array::RecordBatch;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyMemoryError, PyOSError, PyValueError, PyWarning};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use pyo3::types::PyType;
use pyo3_arrow::input::AnyRecordBatch;
use pyo3_arrow::{PyRecordBatchReader, PySchema, PyTable};
use uniformat_sas::{Compression, Encoding, InvalidBytes, ReadReport, Strictness};
use uniformat_sas_arrow::{
    arrow_schema, BatchReader, DateMode, MissingMode, OutOfRange, ReadOptions, ReadStats, TimeMode,
    TimestampUnit, TrailingBlanks,
};
use uniformat_sas_io::{ByteSource, CountingSource, FileSource, PlanMode, SasFile};

/// Exception classes of the module. They are created at import time so that `SasError` derives from
/// `uniformat_io.core.UniFormatError` (with `PyException` as the fallback root if the core package is missing).
struct Exceptions {
    error: Py<PyType>,
    format: Py<PyType>,
    unsupported: Py<PyType>,
    write: Py<PyType>,
}

static EXCEPTIONS: PyOnceLock<Exceptions> = PyOnceLock::new();

fn build_exceptions(py: Python<'_>) -> PyResult<Exceptions> {
    let base: Bound<'_, PyType> = match py
        .import("uniformat_io.core")
        .and_then(|m| m.getattr("UniFormatError"))
        .and_then(|b| b.cast_into::<PyType>().map_err(PyErr::from))
    {
        Ok(b) => b,
        Err(_) => py.get_type::<PyException>(),
    };
    let error = PyErr::new_type(
        py,
        c"uniformat_io.sas.SasError",
        Some(c"Error of the SAS driver."),
        Some(&base),
        None,
    )?;
    let root = error.bind(py).clone();
    let format = PyErr::new_type(
        py,
        c"uniformat_io.sas.SasFormatError",
        Some(c"Corrupt or truncated file, or not a .sas7bdat file."),
        Some(&root),
        None,
    )?;
    let unsupported = PyErr::new_type(
        py,
        c"uniformat_io.sas.SasUnsupportedError",
        Some(c"Feature or file variant that is not supported."),
        Some(&root),
        None,
    )?;
    let write = PyErr::new_type(
        py,
        c"uniformat_io.sas.SasWriteError",
        Some(c"Write error: invalid schema, unrepresentable value or limit exceeded."),
        Some(&root),
        None,
    )?;
    Ok(Exceptions {
        error,
        format,
        unsupported,
        write,
    })
}

fn exceptions(py: Python<'_>) -> &'static Exceptions {
    EXCEPTIONS.get_or_init(py, || {
        build_exceptions(py).unwrap_or_else(|e| {
            // cannot happen outside of an interpreter that is shutting down: fall back to plain exceptions
            let plain = py.get_type::<PyException>().unbind();
            let _ = e;
            Exceptions {
                error: plain.clone_ref(py),
                format: plain.clone_ref(py),
                unsupported: plain.clone_ref(py),
                write: plain,
            }
        })
    })
}

#[derive(Clone, Copy)]
enum Kind {
    Error,
    Format,
    Unsupported,
    Write,
}

/// A `PyErr` of the given SAS exception class.
fn sas_err(kind: Kind, message: impl ToString) -> PyErr {
    Python::attach(|py| {
        let e = exceptions(py);
        let ty = match kind {
            Kind::Error => &e.error,
            Kind::Format => &e.format,
            Kind::Unsupported => &e.unsupported,
            Kind::Write => &e.write,
        };
        PyErr::from_type(ty.bind(py).clone(), message.to_string())
    })
}

create_exception!(
    uniformat_io.sas,
    SasWarning,
    PyWarning,
    "Read warning (unverified case, value replaced or set to null)."
);

fn core_error(e: uniformat_sas::Error) -> PyErr {
    match e {
        uniformat_sas::Error::UnsupportedFormat { .. } => sas_err(Kind::Unsupported, e.to_string()),
        _ => sas_err(Kind::Format, e.to_string()),
    }
}

fn to_py(e: uniformat_sas_arrow::Error) -> PyErr {
    use uniformat_sas_arrow::Error as E;
    use uniformat_sas_io::{IoError, WriteIoError};
    match e {
        E::Io(IoError::Io(io)) => PyOSError::new_err(io.to_string()).with_io_kind(&io),
        E::Io(IoError::Format(f)) | E::Format(f) => core_error(f),
        E::Projection(_) => PyValueError::new_err(e.to_string()),
        E::ResourceLimit(_) => PyMemoryError::new_err(e.to_string()),
        E::Unsupported(what) => sas_err(Kind::Unsupported, what),
        E::Write(WriteIoError::Io(io)) => PyOSError::new_err(io.to_string()).with_io_kind(&io),
        E::Write(w) => sas_err(Kind::Write, w.to_string()),
        E::UnsupportedType { .. } | E::CharLengthUnknown(_) => sas_err(Kind::Write, e.to_string()),
        other => sas_err(Kind::Error, other.to_string()),
    }
}

/// Type d'exception précis pour les erreurs d'entrée/sortie (`FileNotFoundError`, `PermissionError`…).
trait WithIoKind {
    fn with_io_kind(self, io: &std::io::Error) -> PyErr;
}

impl WithIoKind for PyErr {
    fn with_io_kind(self, io: &std::io::Error) -> PyErr {
        match io.kind() {
            std::io::ErrorKind::NotFound => {
                pyo3::exceptions::PyFileNotFoundError::new_err(io.to_string())
            }
            std::io::ErrorKind::PermissionDenied => {
                pyo3::exceptions::PyPermissionError::new_err(io.to_string())
            }
            _ => self,
        }
    }
}

fn value_error(name: &str, got: &str, allowed: &[&str]) -> PyErr {
    PyValueError::new_err(format!(
        "{name} = {got:?} : valeurs admises {}",
        allowed.join(", ")
    ))
}

/// Options de lecture telles que le module Python les reçoit.
#[derive(Debug, Clone)]
struct PyOptions {
    trailing_blanks: String,
    missing: String,
    missing_suffix: String,
    dates: String,
    timestamp_unit: String,
    time: String,
    out_of_range: String,
    invalid_bytes: String,
    empty_as_null: bool,
    encoding: Option<String>,
    strict: bool,
    batch_rows: usize,
    plan: String,
}

impl PyOptions {
    // les options sont validées une à une : l'ordre des erreurs suit celui des paramètres
    #[allow(clippy::field_reassign_with_default)]
    fn build(&self) -> PyResult<ReadOptions> {
        let mut o = ReadOptions::default();
        o.trailing_blanks = match self.trailing_blanks.as_str() {
            "trim" => TrailingBlanks::Trim,
            "keep" => TrailingBlanks::Keep,
            s => return Err(value_error("trailing_blanks", s, &["trim", "keep"])),
        };
        o.missing = match self.missing.as_str() {
            "null" => MissingMode::Null,
            "sidecar" => MissingMode::Sidecar,
            s => return Err(value_error("missing", s, &["null", "sidecar"])),
        };
        if self.missing_suffix.is_empty() {
            return Err(PyValueError::new_err(
                "missing_suffix ne peut pas être vide",
            ));
        }
        o.missing_suffix.clone_from(&self.missing_suffix);
        o.dates = match self.dates.as_str() {
            "native" => DateMode::Native,
            "float" => DateMode::AsFloat,
            s => return Err(value_error("dates", s, &["native", "float"])),
        };
        o.timestamp_unit = match self.timestamp_unit.as_str() {
            "us" => TimestampUnit::Microsecond,
            "s" => TimestampUnit::Second,
            s => return Err(value_error("timestamp_unit", s, &["us", "s"])),
        };
        o.time_mode = match self.time.as_str() {
            "time" => TimeMode::Time64,
            "duration" => TimeMode::Duration,
            s => return Err(value_error("time", s, &["time", "duration"])),
        };
        o.out_of_range = match self.out_of_range.as_str() {
            "null" => OutOfRange::Null,
            "error" => OutOfRange::Error,
            s => return Err(value_error("out_of_range", s, &["null", "error"])),
        };
        o.invalid_bytes = match self.invalid_bytes.as_str() {
            "replace" => InvalidBytes::Replace,
            "error" => InvalidBytes::Error,
            "binary" => InvalidBytes::Binary,
            s => {
                return Err(value_error(
                    "invalid_bytes",
                    s,
                    &["replace", "error", "binary"],
                ))
            }
        };
        o.empty_as_null = self.empty_as_null;
        o.encoding_override = match &self.encoding {
            None => None,
            Some(label) => Some(
                Encoding::for_label(label.as_bytes())
                    .ok_or_else(|| PyValueError::new_err(format!("encoding {label:?} inconnu")))?,
            ),
        };
        o.strictness = if self.strict {
            Strictness::Strict
        } else {
            Strictness::Lenient
        };
        if self.batch_rows == 0 {
            return Err(PyValueError::new_err("batch_rows doit être positif"));
        }
        o.batch_rows = self.batch_rows;
        o.plan = match self.plan.as_str() {
            "fast" => PlanMode::Fast,
            "full" => PlanMode::Full,
            s => return Err(value_error("plan", s, &["fast", "full"])),
        };
        Ok(o)
    }
}

/// Chemin local (`file://` accepté) ; les URL distantes ne sont pas prises en charge par cette version.
fn local_path(path: &str) -> PyResult<&str> {
    if let Some(rest) = path.strip_prefix("file://") {
        return Ok(rest);
    }
    if path.contains("://") {
        return Err(sas_err(
            Kind::Unsupported,
            "URL distante non prise en charge par le module Python (chemin local seulement)",
        ));
    }
    Ok(path)
}

fn open_file<S: ByteSource>(source: S, opts: &ReadOptions) -> PyResult<SasFile<S>> {
    SasFile::open(source, opts.open_options()).map_err(|e| to_py(e.into()))
}

fn open_path(path: &str, opts: &ReadOptions) -> PyResult<Arc<SasFile<FileSource>>> {
    let source = FileSource::open(local_path(path)?)
        .map_err(|e| to_py(uniformat_sas_io::IoError::Io(e).into()))?;
    Ok(Arc::new(open_file(source, opts)?))
}

fn emit(py: Python<'_>, message: &str) -> PyResult<()> {
    let text = CString::new(message.replace('\0', " "))
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    PyErr::warn(py, &py.get_type::<SasWarning>(), &text, 2)
}

fn emit_report(py: Python<'_>, report: &ReadReport) -> PyResult<()> {
    for w in report.warnings() {
        match w.page {
            Some(p) => emit(py, &format!("{} (page {p}) : {}", w.id, w.detail))?,
            None => emit(py, &format!("{} : {}", w.id, w.detail))?,
        }
    }
    if report.dropped() > 0 {
        emit(
            py,
            &format!(
                "{} avertissements supplémentaires non conservés",
                report.dropped()
            ),
        )?;
    }
    Ok(())
}

fn emit_stats(py: Python<'_>, stats: &ReadStats) -> PyResult<()> {
    if stats.out_of_range > 0 {
        emit(
            py,
            &format!(
                "{} valeurs de date, date-heure ou heure hors plage ont été rendues nulles (out_of_range=\"error\" pour échouer)",
                stats.out_of_range
            ),
        )?;
    }
    if stats.replaced_text > 0 {
        emit(
            py,
            &format!(
                "{} champs texte contiennent des octets invalides pour l'encodage, remplacés par U+FFFD (invalid_bytes=\"binary\" pour les conserver)",
                stats.replaced_text
            ),
        )?;
    }
    Ok(())
}

/// Indices des colonnes demandées dans le schéma complet (sidecars compris, selon `missing`).
fn projection(
    meta: &uniformat_sas::TableMeta,
    opts: &ReadOptions,
    columns: Option<Vec<String>>,
) -> PyResult<Option<Vec<usize>>> {
    let Some(columns) = columns else {
        return Ok(None);
    };
    let schema = arrow_schema(meta, opts);
    let mut out = Vec::with_capacity(columns.len());
    for name in columns {
        let Ok(i) = schema.index_of(&name) else {
            return Err(PyValueError::new_err(format!(
                "colonne {name:?} absente de la table"
            )));
        };
        out.push(i);
    }
    Ok(Some(out))
}

fn make_reader(
    path: &str,
    columns: Option<Vec<String>>,
    limit: Option<u64>,
    opts: &ReadOptions,
) -> PyResult<(Arc<SasFile<FileSource>>, BatchReader<FileSource>)> {
    let file = open_path(path, opts)?;
    let proj = projection(file.meta(), opts, columns)?;
    let pages = 0..file.meta().header.page_count;
    let reader =
        BatchReader::new(Arc::clone(&file), pages, proj, limit, opts.clone()).map_err(to_py)?;
    Ok((file, reader))
}

/// `read_sas` : flux Arrow paresseux (interface PyCapsule `__arrow_c_stream__`).
#[pyfunction]
#[pyo3(signature = (
    path, *, columns=None, limit=None, trailing_blanks="trim", missing="null", missing_suffix="__missing",
    dates="native", timestamp_unit="us", time="time", out_of_range="null", invalid_bytes="replace",
    empty_as_null=false, encoding=None, strict=false, batch_rows=8192, plan="fast"
))]
#[allow(clippy::too_many_arguments)]
fn read_sas(
    py: Python<'_>,
    path: &str,
    columns: Option<Vec<String>>,
    limit: Option<u64>,
    trailing_blanks: &str,
    missing: &str,
    missing_suffix: &str,
    dates: &str,
    timestamp_unit: &str,
    time: &str,
    out_of_range: &str,
    invalid_bytes: &str,
    empty_as_null: bool,
    encoding: Option<String>,
    strict: bool,
    batch_rows: usize,
    plan: &str,
) -> PyResult<PyRecordBatchReader> {
    let opts = PyOptions {
        trailing_blanks: trailing_blanks.to_owned(),
        missing: missing.to_owned(),
        missing_suffix: missing_suffix.to_owned(),
        dates: dates.to_owned(),
        timestamp_unit: timestamp_unit.to_owned(),
        time: time.to_owned(),
        out_of_range: out_of_range.to_owned(),
        invalid_bytes: invalid_bytes.to_owned(),
        empty_as_null,
        encoding,
        strict,
        batch_rows,
        plan: plan.to_owned(),
    }
    .build()?;
    let path = path.to_owned();
    let (file, reader) = py.detach(|| make_reader(&path, columns, limit, &opts))?;
    emit_report(py, file.report())?;
    Ok(PyRecordBatchReader::new(Box::new(reader.into_arrow())))
}

/// `read_table` : lit tout, émet les avertissements de lecture, rend une table Arrow.
#[pyfunction]
#[pyo3(signature = (
    path, *, columns=None, limit=None, trailing_blanks="trim", missing="null", missing_suffix="__missing",
    dates="native", timestamp_unit="us", time="time", out_of_range="null", invalid_bytes="replace",
    empty_as_null=false, encoding=None, strict=false, batch_rows=8192, plan="fast", max_table_bytes=268435456
))]
#[allow(clippy::too_many_arguments)]
fn read_table(
    py: Python<'_>,
    path: &str,
    columns: Option<Vec<String>>,
    limit: Option<u64>,
    trailing_blanks: &str,
    missing: &str,
    missing_suffix: &str,
    dates: &str,
    timestamp_unit: &str,
    time: &str,
    out_of_range: &str,
    invalid_bytes: &str,
    empty_as_null: bool,
    encoding: Option<String>,
    strict: bool,
    batch_rows: usize,
    plan: &str,
    max_table_bytes: usize,
) -> PyResult<PyTable> {
    if max_table_bytes == 0 {
        return Err(PyValueError::new_err("max_table_bytes doit être positif"));
    }
    let mut opts = PyOptions {
        trailing_blanks: trailing_blanks.to_owned(),
        missing: missing.to_owned(),
        missing_suffix: missing_suffix.to_owned(),
        dates: dates.to_owned(),
        timestamp_unit: timestamp_unit.to_owned(),
        time: time.to_owned(),
        out_of_range: out_of_range.to_owned(),
        invalid_bytes: invalid_bytes.to_owned(),
        empty_as_null,
        encoding,
        strict,
        batch_rows,
        plan: plan.to_owned(),
    }
    .build()?;
    opts.max_batch_bytes = opts.max_batch_bytes.min(max_table_bytes);
    let path = path.to_owned();
    let (open_report, schema, batches, stats, read_report) = py.detach(
        || -> PyResult<(ReadReport, _, Vec<RecordBatch>, ReadStats, Option<ReadReport>)> {
            let (file, mut reader) = make_reader(&path, columns, limit, &opts)?;
            let schema = reader.schema();
            let mut batches = Vec::new();
            let mut retained_bytes = 0usize;
            for b in reader.by_ref() {
                let batch = b.map_err(to_py)?;
                retained_bytes = retained_bytes.checked_add(batch.get_array_memory_size())
                    .filter(|n| *n <= max_table_bytes)
                    .ok_or_else(|| PyMemoryError::new_err("max_table_bytes dépassé ; utiliser read_sas en flux, limit, ou augmenter le budget pour une source fiable"))?;
                batches.try_reserve(1).map_err(|_| PyMemoryError::new_err("réservation de table impossible"))?;
                batches.push(batch);
            }
            Ok((
                file.report().clone(),
                schema,
                batches,
                *reader.stats(),
                reader.report().cloned(),
            ))
        },
    )?;
    emit_report(py, &open_report)?;
    if let Some(r) = &read_report {
        if r != &open_report {
            emit_report(py, r)?;
        }
    }
    emit_stats(py, &stats)?;
    PyTable::try_new(batches, schema)
}

/// `schema` : schéma Arrow complet depuis l'en-tête et les pages de métadonnées (aucune page de données).
#[pyfunction]
#[pyo3(signature = (path, *, missing="null", missing_suffix="__missing", dates="native",
    timestamp_unit="us", time="time", invalid_bytes="replace", encoding=None, strict=false))]
#[allow(clippy::too_many_arguments)]
fn schema(
    py: Python<'_>,
    path: &str,
    missing: &str,
    missing_suffix: &str,
    dates: &str,
    timestamp_unit: &str,
    time: &str,
    invalid_bytes: &str,
    encoding: Option<String>,
    strict: bool,
) -> PyResult<PySchema> {
    let opts = PyOptions {
        trailing_blanks: "trim".to_owned(),
        missing: missing.to_owned(),
        missing_suffix: missing_suffix.to_owned(),
        dates: dates.to_owned(),
        timestamp_unit: timestamp_unit.to_owned(),
        time: time.to_owned(),
        out_of_range: "null".to_owned(),
        invalid_bytes: invalid_bytes.to_owned(),
        empty_as_null: false,
        encoding,
        strict,
        batch_rows: 8192,
        plan: "fast".to_owned(),
    }
    .build()?;
    let path = path.to_owned();
    let (schema, report) = py.detach(|| -> PyResult<_> {
        let file = open_path(&path, &opts)?;
        Ok((arrow_schema(file.meta(), &opts), file.report().clone()))
    })?;
    emit_report(py, &report)?;
    Ok(PySchema::new(schema))
}

/// `write_table` : écrit un flux Arrow (table, lots, `RecordBatchReader`) dans `path`, de façon transactionnelle.
///
/// Les métadonnées des champs (`sas.label`, `sas.format`, `sas.length`…) décrivent les colonnes SAS ; les longueurs
/// des colonnes caractère sont mesurées sur les données si elles ne sont pas données (le flux est alors lu en
/// mémoire). Rend le rapport d'écriture (dictionnaire).
#[pyfunction]
#[pyo3(signature = (
    path, data, *, compression="none", encoding="utf-8", page_length=65536, grow_page=true,
    on_unrepresentable="error", missing_suffix="__missing", table_name=None, file_code=None, created=None,
    char_lengths=None
))]
#[allow(clippy::too_many_arguments)]
fn write_table<'py>(
    py: Python<'py>,
    path: &str,
    data: AnyRecordBatch,
    compression: &str,
    encoding: &str,
    page_length: u32,
    grow_page: bool,
    on_unrepresentable: &str,
    missing_suffix: &str,
    table_name: Option<String>,
    file_code: Option<u32>,
    created: Option<f64>,
    char_lengths: Option<std::collections::HashMap<String, u32>>,
) -> PyResult<Bound<'py, PyDict>> {
    use uniformat_sas::write::{Unrepresentable, WriteEncoding};
    use uniformat_sas_arrow::{infer_char_lengths, ArrowWriteOptions, ArrowWriter};
    let mut opts = ArrowWriteOptions::default();
    opts.write.compression = match compression {
        "none" => Compression::None,
        "char" => Compression::Char,
        "binary" => Compression::Binary,
        s => return Err(value_error("compression", s, &["none", "char", "binary"])),
    };
    opts.write.encoding = match encoding.to_ascii_lowercase().as_str() {
        "utf-8" | "utf8" => WriteEncoding::Utf8,
        "latin1" | "latin-1" | "iso-8859-1" => WriteEncoding::Latin1,
        "wlatin1" | "cp1252" | "windows-1252" => WriteEncoding::Wlatin1,
        s => {
            return Err(value_error(
                "encoding",
                s,
                &["utf-8", "latin1", "wlatin1 (cp1252)"],
            ))
        }
    };
    opts.write.on_unrepresentable = match on_unrepresentable {
        "error" => Unrepresentable::Error,
        "replace" => Unrepresentable::Replace,
        "truncate" => Unrepresentable::Truncate,
        s => {
            return Err(value_error(
                "on_unrepresentable",
                s,
                &["error", "replace", "truncate"],
            ))
        }
    };
    opts.write.page_length = page_length;
    opts.write.grow_page = grow_page;
    opts.missing_suffix = missing_suffix.to_owned();
    if let Some(n) = table_name {
        opts.write.table_name = n;
    }
    if let Some(c) = file_code {
        opts.write.file_code = c;
    }
    if let Some(t) = created {
        opts.write.created = t + 315_619_200.0;
    }
    opts.char_lengths = char_lengths.unwrap_or_default();
    let reader = data.into_reader()?;
    let schema = reader.schema();
    let path = path.to_owned();
    let report = py.detach(|| -> PyResult<uniformat_sas::write::WriteReport> {
        let needs_scan = schema.fields().iter().any(|f| {
            matches!(
                f.data_type(),
                arrow_schema::DataType::Utf8
                    | arrow_schema::DataType::LargeUtf8
                    | arrow_schema::DataType::Utf8View
                    | arrow_schema::DataType::Binary
                    | arrow_schema::DataType::LargeBinary
                    | arrow_schema::DataType::Dictionary(_, _)
            ) && !f.metadata().contains_key("sas.length")
                && !opts.char_lengths.contains_key(f.name())
        });
        if needs_scan {
            let batches: Vec<RecordBatch> = reader
                .collect::<Result<_, arrow_schema::ArrowError>>()
                .map_err(|e| to_py(uniformat_sas_arrow::Error::Arrow(e)))?;
            for (name, len) in infer_char_lengths(&batches, opts.write.encoding) {
                opts.char_lengths.entry(name).or_insert(len);
            }
            let mut w = ArrowWriter::create(&path, schema, opts).map_err(to_py)?;
            for b in &batches {
                w.write(b).map_err(to_py)?;
            }
            w.commit().map_err(to_py)
        } else {
            let mut w = ArrowWriter::create(&path, schema, opts).map_err(to_py)?;
            for b in reader {
                let b = b.map_err(|e| to_py(uniformat_sas_arrow::Error::Arrow(e)))?;
                w.write(&b).map_err(to_py)?;
            }
            w.commit().map_err(to_py)
        }
    })?;
    for w in &report.warnings {
        emit(py, w)?;
    }
    let out = PyDict::new(py);
    out.set_item("rows", report.rows)?;
    out.set_item("truncated_values", report.truncated_values)?;
    out.set_item("replaced_chars", report.replaced_chars)?;
    out.set_item("replaced_values", report.replaced_values)?;
    out.set_item("inexact_integers", report.inexact_integers)?;
    out.set_item("narrowed_values", report.narrowed_values)?;
    out.set_item("warnings", report.warnings.clone())?;
    Ok(out)
}

/// Contrôle interne de l'ouverture : `(octets lus, longueur d'en-tête, longueur de page, pages de métadonnées lues)`.
///
/// Sert au test « le schéma ne lit aucune page de données » : octets lus ≤ en-tête + pages de métadonnées × page.
#[pyfunction]
fn _open_probe(path: &str) -> PyResult<(u64, u64, u64, u64)> {
    let source = FileSource::open(local_path(path)?)
        .map_err(|e| to_py(uniformat_sas_io::IoError::Io(e).into()))?;
    let counting = CountingSource::new(source);
    let file = open_file(counting, &ReadOptions::default())?;
    let meta_pages = file
        .scanned_pages()
        .iter()
        .filter(|p| {
            !matches!(
                p.kind,
                uniformat_sas::PageKind::Data | uniformat_sas::PageKind::Compressed
            )
        })
        .count();
    let h = &file.meta().header;
    Ok((
        file.source().bytes_read(),
        u64::from(h.header_length),
        u64::from(h.page_length),
        meta_pages as u64,
    ))
}

/// Version du module natif.
#[pyfunction]
fn _version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(read_sas, m)?)?;
    m.add_function(wrap_pyfunction!(read_table, m)?)?;
    m.add_function(wrap_pyfunction!(schema, m)?)?;
    m.add_function(wrap_pyfunction!(write_table, m)?)?;
    m.add_function(wrap_pyfunction!(_open_probe, m)?)?;
    m.add_function(wrap_pyfunction!(_version, m)?)?;
    let py = m.py();
    let e = exceptions(py);
    m.add("SasError", e.error.clone_ref(py))?;
    m.add("SasFormatError", e.format.clone_ref(py))?;
    m.add("SasUnsupportedError", e.unsupported.clone_ref(py))?;
    m.add("SasWriteError", e.write.clone_ref(py))?;
    m.add("SasWarning", m.py().get_type::<SasWarning>())?;
    Ok(())
}

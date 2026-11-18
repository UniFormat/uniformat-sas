//! XPORT uses the common format adapter; data stays in Arrow buffers across Python.
use pyo3::{exceptions::PyValueError, prelude::*};
use pyo3_arrow::{input::AnyRecordBatch, PyRecordBatchReader, PySchema};
use std::{collections::BTreeMap, path::Path};
use uniformat_core::{Format, ReadRequest};
use uniformat_sas_arrow::xpt::{write_options, write_xpt_batches, XptFormat};

fn error(e: uniformat_core::Error) -> PyErr {
    match e {
        uniformat_core::Error::InvalidOption { .. } => PyValueError::new_err(e.to_string()),
        uniformat_core::Error::Io(io) => {
            use crate::WithIoKind;
            pyo3::exceptions::PyOSError::new_err(io.to_string()).with_io_kind(&io)
        }
        _ => crate::sas_err(crate::Kind::Format, e.to_string()),
    }
}
#[pyfunction]
#[pyo3(signature=(path, options, columns=None, limit=None))]
fn read_xpt(
    py: Python<'_>,
    path: &str,
    options: BTreeMap<String, String>,
    columns: Option<Vec<String>>,
    limit: Option<u64>,
) -> PyResult<PyRecordBatchReader> {
    let path = crate::local_path(path)?.to_owned();
    let reader = py.detach(|| {
        XptFormat
            .read(
                Path::new(&path),
                &ReadRequest {
                    options,
                    columns,
                    limit,
                },
            )
            .map_err(error)
    })?;
    Ok(PyRecordBatchReader::new(reader))
}
#[pyfunction]
fn xpt_schema(py: Python<'_>, path: &str, options: BTreeMap<String, String>) -> PyResult<PySchema> {
    let path = crate::local_path(path)?.to_owned();
    let info = py.detach(|| {
        XptFormat
            .info(
                Path::new(&path),
                &ReadRequest {
                    options,
                    ..Default::default()
                },
            )
            .map_err(error)
    })?;
    Ok(PySchema::new(info.schema))
}
#[pyfunction]
fn write_xpt(
    py: Python<'_>,
    path: &str,
    data: AnyRecordBatch,
    options: BTreeMap<String, String>,
) -> PyResult<u64> {
    let path = crate::local_path(path)?.to_owned();
    let opts = write_options(&options).map_err(error)?;
    let reader = data.into_reader()?;
    let schema = reader.schema();
    py.detach(|| write_xpt_batches(path, schema, reader, opts).map_err(crate::to_py))
}
pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(read_xpt, m)?)?;
    m.add_function(wrap_pyfunction!(xpt_schema, m)?)?;
    m.add_function(wrap_pyfunction!(write_xpt, m)?)?;
    Ok(())
}

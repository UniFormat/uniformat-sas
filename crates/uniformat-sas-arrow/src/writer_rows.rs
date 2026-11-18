//! Arrow-to-SAS cells shared by sas7bdat and XPORT writers.
use crate::writer::{is_numeric_type, missing_from_code, numeric_cell, plan, FieldPlan, Num};
use crate::{ArrowWriteOptions, Error, Result};
use arrow_array::{cast::AsArray, types::UInt8Type, ArrayRef, RecordBatch};
use arrow_schema::{DataType, SchemaRef};
use uniformat_sas::write::{CellProblem, CellValue, Unrepresentable, WriteError, WriteSchema};
use uniformat_sas_io::WriteIoError;

pub(crate) struct RowEncoder {
    schema: SchemaRef,
    plan: Vec<FieldPlan>,
    policy: Unrepresentable,
    pub(crate) rows: u64,
    strict: bool,
}
impl RowEncoder {
    pub(crate) fn new(
        schema: SchemaRef,
        opts: &ArrowWriteOptions,
        strict: bool,
    ) -> Result<(WriteSchema, Self)> {
        let (ws, plan) = plan(&schema, opts)?;
        Ok((
            ws,
            Self {
                schema,
                plan,
                policy: opts.write.on_unrepresentable,
                rows: 0,
                strict,
            },
        ))
    }
    pub(crate) fn write(
        &mut self,
        batch: &RecordBatch,
        mut push: impl FnMut(&[CellValue<'_>], u64) -> Result<()>,
    ) -> Result<()> {
        if batch.schema().fields() != self.schema.fields() {
            return Err(Error::Unsupported("le lot n'a pas le schéma de l'écrivain"));
        }
        let cols: Vec<ArrayRef> = batch
            .columns()
            .iter()
            .map(|c| match c.data_type() {
                DataType::Dictionary(_, v) => arrow_cast::cast(c, v).map_err(Error::Arrow),
                _ => Ok(c.clone()),
            })
            .collect::<Result<_>>()?;
        let n_out = self.plan.iter().filter(|p| p.out.is_some()).count();
        for i in 0..batch.num_rows() {
            let mut cells: Vec<CellValue<'_>> = Vec::with_capacity(n_out);
            let mut inexact = 0u64;
            for (fi, p) in self.plan.iter().enumerate() {
                let Some(out) = p.out else { continue };
                let Some(col) = cols.get(fi) else { continue };
                let cell = if is_numeric_type(col.data_type()) {
                    let sidecar = p
                        .sidecar
                        .and_then(|s| cols.get(s))
                        .filter(|c| !c.is_null(i))
                        .map(|c| c.as_primitive::<UInt8Type>().value(i));
                    if self.strict && sidecar.is_some_and(|code| code > 28) {
                        return Err(Error::Unsupported(
                            "XPT missing sidecar code must be in 0..28",
                        ));
                    }
                    match sidecar.and_then(missing_from_code) {
                        Some(m) => CellValue::Missing(m),
                        None => match numeric_cell(col, i) {
                            Num::Null => CellValue::Null,
                            Num::Value(v) => CellValue::Num(v),
                            Num::Inexact(v) => match self.policy {
                                Unrepresentable::Replace => {
                                    inexact = inexact.saturating_add(1);
                                    CellValue::Num(v)
                                }
                                _ => return Err(self.cell_error(out, CellProblem::IntegerTooLarge)),
                            },
                            Num::Bad(CellProblem::OutOfRange) => match self.policy {
                                Unrepresentable::Replace => CellValue::Null,
                                _ => return Err(self.cell_error(out, CellProblem::OutOfRange)),
                            },
                            Num::Bad(p) => return Err(self.cell_error(out, p)),
                        },
                    }
                } else if col.is_null(i) {
                    CellValue::Null
                } else {
                    match col.data_type() {
                        DataType::Utf8 => CellValue::Text(col.as_string::<i32>().value(i)),
                        DataType::LargeUtf8 => CellValue::Text(col.as_string::<i64>().value(i)),
                        DataType::Utf8View => CellValue::Text(col.as_string_view().value(i)),
                        DataType::Binary => CellValue::Bytes(col.as_binary::<i32>().value(i)),
                        DataType::LargeBinary => CellValue::Bytes(col.as_binary::<i64>().value(i)),
                        DataType::FixedSizeBinary(_) => {
                            CellValue::Bytes(col.as_fixed_size_binary().value(i))
                        }
                        _ => return Err(self.cell_error(out, CellProblem::WrongType)),
                    }
                };
                cells.push(cell);
            }
            push(&cells, inexact)?;
            self.rows = self.rows.saturating_add(1);
        }
        Ok(())
    }

    fn cell_error(&self, column: usize, problem: CellProblem) -> Error {
        Error::Write(WriteIoError::Write(WriteError::Cell {
            column,
            row: self.rows,
            problem,
        }))
    }
}

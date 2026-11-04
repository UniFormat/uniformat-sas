//! `ArrowWriter` : écrit des `RecordBatch` Arrow dans un fichier `.sas7bdat` (ADR 0005 §1 et §4, ADR 0003).
//!
//! Types acceptés : `Float32/64`, entiers, `Boolean`, `Decimal128`, `Date32/64`, `Timestamp`, `Time32/64`,
//! `Duration` (numériques) ; `Utf8`, `LargeUtf8`, `Utf8View`, `Binary`, `LargeBinary`, `FixedSizeBinary` (caractères) ;
//! `Dictionary` de ces types. Une colonne `x__missing` (`UInt8`, ADR 0003) à côté d'une colonne numérique `x` porte le
//! code de manquant spécial et n'est pas écrite comme colonne.

use std::collections::HashMap;
use std::io::{Seek, Write};
use std::path::Path;

use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Date64Type, Decimal128Type, DurationMicrosecondType, DurationMillisecondType,
    DurationNanosecondType, DurationSecondType, Float32Type, Float64Type, Int16Type, Int32Type,
    Int64Type, Int8Type, Time32MillisecondType, Time32SecondType, Time64MicrosecondType,
    Time64NanosecondType, TimestampMicrosecondType, TimestampMillisecondType,
    TimestampNanosecondType, TimestampSecondType, UInt16Type, UInt32Type, UInt64Type, UInt8Type,
};
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_schema::{DataType, SchemaRef, TimeUnit};
use uniformat_sas::temporal::{max_day, DAYS_1960_TO_1970, MIN_DAY, SECONDS_1960_TO_1970};
use uniformat_sas::write::{
    CellProblem, CellValue, Unrepresentable, WriteColumn, WriteEncoding, WriteError, WriteOptions,
    WriteReport, WriteSchema,
};
use uniformat_sas::MissingKind;
use uniformat_sas_io::{AtomicFile, SasWriter, WriteIoError};

use crate::error::{Error, Result};

/// Options de l'écrivain Arrow.
#[derive(Debug, Clone)]
pub struct ArrowWriteOptions {
    /// Options du cœur (encodage, page, compression, valeurs non représentables…).
    pub write: WriteOptions,
    /// Suffixe de la colonne annexe des manquants spéciaux.
    pub missing_suffix: String,
    /// Longueur (octets) des colonnes caractère, par nom ; à défaut la métadonnée `sas.length` du champ.
    pub char_lengths: HashMap<String, u32>,
}

impl Default for ArrowWriteOptions {
    fn default() -> Self {
        Self {
            write: uniformat_sas_io::system_options(),
            missing_suffix: "__missing".to_owned(),
            char_lengths: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Column,
    Sidecar,
}

#[derive(Debug, Clone)]
struct FieldPlan {
    role: Role,
    /// Colonne SAS correspondante (rang dans `WriteSchema`) pour `Column`.
    out: Option<usize>,
    /// Pour une colonne numérique : rang du champ Arrow de sa colonne annexe.
    sidecar: Option<usize>,
}

fn metadata_u16(f: &arrow_schema::Field, key: &str) -> u16 {
    f.metadata()
        .get(key)
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(0)
}

fn is_numeric_type(t: &DataType) -> bool {
    use DataType as T;
    matches!(
        t,
        T::Float16
            | T::Float32
            | T::Float64
            | T::Int8
            | T::Int16
            | T::Int32
            | T::Int64
            | T::UInt8
            | T::UInt16
            | T::UInt32
            | T::UInt64
            | T::Boolean
            | T::Decimal128(_, _)
            | T::Date32
            | T::Date64
            | T::Timestamp(_, _)
            | T::Time32(_)
            | T::Time64(_)
            | T::Duration(_)
    )
}

fn is_text_type(t: &DataType) -> bool {
    use DataType as T;
    matches!(
        t,
        T::Utf8 | T::LargeUtf8 | T::Utf8View | T::Binary | T::LargeBinary | T::FixedSizeBinary(_)
    )
}

fn base_type(t: &DataType) -> &DataType {
    match t {
        DataType::Dictionary(_, v) => base_type(v),
        other => other,
    }
}

/// Octets de `s` dans l'encodage cible, sans alerte (pour mesurer les colonnes).
fn encoded_len(enc: WriteEncoding, s: &str) -> usize {
    enc.encode_lossy(s).0.len()
}

/// Longueur maximale (octets, dans l'encodage cible) des colonnes caractère d'un ensemble de lots.
#[must_use]
pub fn infer_char_lengths(batches: &[RecordBatch], enc: WriteEncoding) -> HashMap<String, u32> {
    let mut out: HashMap<String, u32> = HashMap::new();
    for b in batches {
        for (i, f) in b.schema().fields().iter().enumerate() {
            if !is_text_type(base_type(f.data_type())) {
                continue;
            }
            let plain = match b.column(i).data_type() {
                DataType::Dictionary(_, v) => arrow_cast::cast(b.column(i), v).ok(),
                _ => Some(b.column(i).clone()),
            };
            let Some(col) = plain else { continue };
            let mut max = 0usize;
            match col.data_type() {
                DataType::Utf8 => {
                    for v in col.as_string::<i32>().iter().flatten() {
                        max = max.max(encoded_len(enc, v));
                    }
                }
                DataType::LargeUtf8 => {
                    for v in col.as_string::<i64>().iter().flatten() {
                        max = max.max(encoded_len(enc, v));
                    }
                }
                DataType::Utf8View => {
                    for v in col.as_string_view().iter().flatten() {
                        max = max.max(encoded_len(enc, v));
                    }
                }
                DataType::Binary => {
                    for v in col.as_binary::<i32>().iter().flatten() {
                        max = max.max(v.len());
                    }
                }
                DataType::LargeBinary => {
                    for v in col.as_binary::<i64>().iter().flatten() {
                        max = max.max(v.len());
                    }
                }
                _ => {}
            }
            let max = u32::try_from(max).unwrap_or(u32::MAX).max(1);
            let e = out.entry(f.name().clone()).or_insert(1);
            *e = (*e).max(max);
        }
    }
    out
}

/// Schéma SAS déduit d'un schéma Arrow : colonnes, longueurs, labels, formats (métadonnées `sas.*`, ADR 0003).
///
/// # Errors
/// Type Arrow non pris en charge, longueur de colonne caractère inconnue.
pub fn write_schema_from_arrow(
    schema: &SchemaRef,
    opts: &ArrowWriteOptions,
) -> Result<WriteSchema> {
    let (ws, _) = plan(schema, opts)?;
    Ok(ws)
}

fn plan(schema: &SchemaRef, opts: &ArrowWriteOptions) -> Result<(WriteSchema, Vec<FieldPlan>)> {
    let fields = schema.fields();
    let mut roles: Vec<FieldPlan> = fields
        .iter()
        .map(|_| FieldPlan {
            role: Role::Column,
            out: None,
            sidecar: None,
        })
        .collect();
    // colonnes annexes : `x__missing` en `UInt8` à côté d'une colonne numérique `x`
    for (i, f) in fields.iter().enumerate() {
        let pair = match f.metadata().get("sas.missing_sidecar") {
            Some(name) => fields.iter().position(|g| g.name() == name).map(|s| (i, s)),
            None if f.data_type() == &DataType::UInt8 => f
                .name()
                .strip_suffix(opts.missing_suffix.as_str())
                .and_then(|base| fields.iter().position(|g| g.name() == base))
                .map(|n| (n, i)),
            None => None,
        };
        let Some((num, side)) = pair else { continue };
        let (Some(nf), Some(sf)) = (fields.get(num), fields.get(side)) else {
            continue;
        };
        if is_numeric_type(base_type(nf.data_type())) && sf.data_type() == &DataType::UInt8 {
            if let Some(p) = roles.get_mut(side) {
                p.role = Role::Sidecar;
            }
            if let Some(p) = roles.get_mut(num) {
                p.sidecar = Some(side);
            }
        }
    }
    let mut columns = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        if roles.get(i).map(|p| p.role) == Some(Role::Sidecar) {
            continue;
        }
        let t = base_type(f.data_type());
        let name = f.name().clone();
        let mut col = if is_numeric_type(t) {
            let length = match f.metadata().get("sas.length") {
                Some(v) => v
                    .parse::<u8>()
                    .map_err(|_| Error::Unsupported("sas.length numérique invalide"))?,
                None => 8,
            };
            WriteColumn::numeric(name.clone(), length)
        } else if is_text_type(t) {
            let length = f
                .metadata()
                .get("sas.length")
                .and_then(|v| v.parse::<u32>().ok())
                .or_else(|| opts.char_lengths.get(&name).copied())
                .ok_or(Error::CharLengthUnknown(name.clone()))?;
            WriteColumn::character(name.clone(), length)
        } else {
            return Err(Error::UnsupportedType {
                column: name,
                data_type: f.data_type().to_string(),
            });
        };
        if let Some(l) = f.metadata().get("sas.label") {
            col.label.clone_from(l);
        }
        if let Some(n) = f.metadata().get("sas.format") {
            col = col.with_format(
                n.clone(),
                metadata_u16(f, "sas.format_width"),
                metadata_u16(f, "sas.format_decimals"),
            );
        } else {
            let default = match t {
                DataType::Date32 | DataType::Date64 => Some(("DATE", 9u16)),
                DataType::Timestamp(_, _) => Some(("DATETIME", 20)),
                DataType::Time32(_) | DataType::Time64(_) | DataType::Duration(_) => {
                    Some(("TIME", 8))
                }
                _ => None,
            };
            if let Some((n, w)) = default {
                col = col.with_format(n, w, 0);
            }
        }
        if let Some(n) = f.metadata().get("sas.informat") {
            col = col.with_informat(
                n.clone(),
                metadata_u16(f, "sas.informat_width"),
                metadata_u16(f, "sas.informat_decimals"),
            );
        }
        if let Some(p) = roles.get_mut(i) {
            p.out = Some(columns.len());
        }
        columns.push(col);
    }
    Ok((WriteSchema { columns }, roles))
}

fn missing_from_code(code: u8) -> Option<MissingKind> {
    match code {
        0 => None,
        1 => Some(MissingKind::Dot),
        2 => Some(MissingKind::Underscore),
        3..=28 => Some(MissingKind::Letter(b'A'.wrapping_add(code.wrapping_sub(3)))),
        _ => Some(MissingKind::Dot),
    }
}

const TWO_53: i64 = 1 << 53;

/// Valeur numérique (ou manquant) d'une cellule d'un tableau Arrow, en secondes/jours SAS pour les dates.
enum Num {
    Null,
    Value(f64),
    /// Valeur arrondie (entier de plus de 53 bits ou décimal inexact) : compteur `inexact_integers`.
    Inexact(f64),
    Bad(CellProblem),
}

fn day_in_range(sas_days: f64) -> bool {
    sas_days.is_finite() && sas_days >= MIN_DAY as f64 && sas_days < (max_day() as f64) + 1.0
}

fn unit_div(u: &TimeUnit) -> f64 {
    match u {
        TimeUnit::Second => 1.0,
        TimeUnit::Millisecond => 1e3,
        TimeUnit::Microsecond => 1e6,
        TimeUnit::Nanosecond => 1e9,
    }
}

fn int_num(v: i64) -> Num {
    if v.unsigned_abs() > (TWO_53 as u64) {
        Num::Inexact(v as f64)
    } else {
        Num::Value(v as f64)
    }
}

fn numeric_cell(col: &ArrayRef, i: usize) -> Num {
    if col.is_null(i) {
        return Num::Null;
    }
    match col.data_type() {
        DataType::Float64 => Num::Value(col.as_primitive::<Float64Type>().value(i)),
        DataType::Float32 => Num::Value(f64::from(col.as_primitive::<Float32Type>().value(i))),
        DataType::Float16 => Num::Value(f64::from(
            col.as_primitive::<arrow_array::types::Float16Type>()
                .value(i),
        )),
        DataType::Int8 => Num::Value(f64::from(col.as_primitive::<Int8Type>().value(i))),
        DataType::Int16 => Num::Value(f64::from(col.as_primitive::<Int16Type>().value(i))),
        DataType::Int32 => Num::Value(f64::from(col.as_primitive::<Int32Type>().value(i))),
        DataType::Int64 => int_num(col.as_primitive::<Int64Type>().value(i)),
        DataType::UInt8 => Num::Value(f64::from(col.as_primitive::<UInt8Type>().value(i))),
        DataType::UInt16 => Num::Value(f64::from(col.as_primitive::<UInt16Type>().value(i))),
        DataType::UInt32 => Num::Value(f64::from(col.as_primitive::<UInt32Type>().value(i))),
        DataType::UInt64 => {
            let v = col.as_primitive::<UInt64Type>().value(i);
            if v > (TWO_53 as u64) {
                Num::Inexact(v as f64)
            } else {
                Num::Value(v as f64)
            }
        }
        DataType::Boolean => Num::Value(f64::from(u8::from(col.as_boolean().value(i)))),
        DataType::Decimal128(_, scale) => {
            let v = col.as_primitive::<Decimal128Type>().value(i);
            let x = v as f64 / 10f64.powi(i32::from(*scale));
            if v.unsigned_abs() > (TWO_53 as u128) {
                Num::Inexact(x)
            } else {
                Num::Value(x)
            }
        }
        DataType::Date32 => {
            let d = f64::from(col.as_primitive::<Date32Type>().value(i)) + DAYS_1960_TO_1970 as f64;
            if day_in_range(d) {
                Num::Value(d)
            } else {
                Num::Bad(CellProblem::OutOfRange)
            }
        }
        DataType::Date64 => {
            let ms = col.as_primitive::<Date64Type>().value(i);
            let d = ms.div_euclid(86_400_000) as f64 + DAYS_1960_TO_1970 as f64;
            if day_in_range(d) {
                Num::Value(d)
            } else {
                Num::Bad(CellProblem::OutOfRange)
            }
        }
        DataType::Timestamp(u, _) => {
            let raw = match u {
                TimeUnit::Second => col.as_primitive::<TimestampSecondType>().value(i),
                TimeUnit::Millisecond => col.as_primitive::<TimestampMillisecondType>().value(i),
                TimeUnit::Microsecond => col.as_primitive::<TimestampMicrosecondType>().value(i),
                TimeUnit::Nanosecond => col.as_primitive::<TimestampNanosecondType>().value(i),
            };
            let secs = raw as f64 / unit_div(u) + SECONDS_1960_TO_1970 as f64;
            if day_in_range((secs / 86_400.0).floor()) {
                Num::Value(secs)
            } else {
                Num::Bad(CellProblem::OutOfRange)
            }
        }
        DataType::Time32(u) => {
            let raw = match u {
                TimeUnit::Second => i64::from(col.as_primitive::<Time32SecondType>().value(i)),
                _ => i64::from(col.as_primitive::<Time32MillisecondType>().value(i)),
            };
            Num::Value(raw as f64 / unit_div(u))
        }
        DataType::Time64(u) => {
            let raw = match u {
                TimeUnit::Nanosecond => col.as_primitive::<Time64NanosecondType>().value(i),
                _ => col.as_primitive::<Time64MicrosecondType>().value(i),
            };
            Num::Value(raw as f64 / unit_div(u))
        }
        DataType::Duration(u) => {
            let raw = match u {
                TimeUnit::Second => col.as_primitive::<DurationSecondType>().value(i),
                TimeUnit::Millisecond => col.as_primitive::<DurationMillisecondType>().value(i),
                TimeUnit::Microsecond => col.as_primitive::<DurationMicrosecondType>().value(i),
                TimeUnit::Nanosecond => col.as_primitive::<DurationNanosecondType>().value(i),
            };
            Num::Value(raw as f64 / unit_div(u))
        }
        _ => Num::Bad(CellProblem::WrongType),
    }
}

/// Écrivain de lots Arrow.
pub struct ArrowWriter<W: Write + Seek> {
    inner: SasWriter<W>,
    schema: SchemaRef,
    plan: Vec<FieldPlan>,
    policy: Unrepresentable,
    rows: u64,
}

impl<W: Write + Seek> std::fmt::Debug for ArrowWriter<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArrowWriter")
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

impl<W: Write + Seek> ArrowWriter<W> {
    /// Valide le schéma **avant le premier octet** puis prépare l'écriture.
    ///
    /// # Errors
    /// Type non pris en charge, longueur caractère inconnue, limite de schéma dépassée.
    pub fn try_new(sink: W, schema: SchemaRef, opts: ArrowWriteOptions) -> Result<Self> {
        let (ws, plan) = self::plan(&schema, &opts)?;
        let policy = opts.write.on_unrepresentable;
        let mut wopts = opts.write;
        if wopts.table_name.is_empty() {
            if let Some(n) = schema.metadata().get("sas.table_name") {
                wopts.table_name.clone_from(n);
            }
        }
        let inner = SasWriter::new(sink, &ws, wopts)?;
        Ok(Self {
            inner,
            schema,
            plan,
            policy,
            rows: 0,
        })
    }

    /// Taille de page finalement utilisée.
    #[must_use]
    pub fn page_length(&self) -> u32 {
        self.inner.page_length()
    }

    /// Écarts comptés jusqu'ici.
    #[must_use]
    pub fn report(&self) -> Option<&WriteReport> {
        self.inner.report()
    }

    /// Écrit un lot.
    ///
    /// # Errors
    /// Schéma différent, valeur non représentable, erreur d'entrée/sortie.
    pub fn write(&mut self, batch: &RecordBatch) -> Result<()> {
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
            self.inner.write_row(&cells)?;
            if inexact > 0 {
                if let Some(r) = self.inner.report_mut() {
                    r.inexact_integers = r.inexact_integers.saturating_add(inexact);
                }
            }
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

    /// Termine le fichier et rend le flux et le rapport.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie.
    pub fn finish(self) -> Result<(W, WriteReport)> {
        Ok(self.inner.finish()?)
    }
}

impl ArrowWriter<AtomicFile> {
    /// Écrit dans `path` de façon transactionnelle (fichier temporaire renommé par [`ArrowWriter::commit`]).
    ///
    /// # Errors
    /// Schéma invalide ou erreur d'entrée/sortie.
    pub fn create(
        path: impl AsRef<Path>,
        schema: SchemaRef,
        opts: ArrowWriteOptions,
    ) -> Result<Self> {
        let (ws, plan) = self::plan(&schema, &opts)?;
        let policy = opts.write.on_unrepresentable;
        let mut wopts = opts.write;
        if wopts.table_name.is_empty() {
            if let Some(n) = schema.metadata().get("sas.table_name") {
                wopts.table_name.clone_from(n);
            }
        }
        let inner = SasWriter::create(path, &ws, wopts)?;
        Ok(Self {
            inner,
            schema,
            plan,
            policy,
            rows: 0,
        })
    }

    /// Termine l'écriture et renomme le fichier temporaire.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie.
    pub fn commit(self) -> Result<WriteReport> {
        Ok(self.inner.commit()?)
    }
}

/// Écrit des lots dans un fichier de façon transactionnelle : les longueurs des colonnes caractère sont mesurées
/// sur les lots si elles ne sont pas connues.
///
/// # Errors
/// Voir [`ArrowWriter`] ; aucun fichier n'est laissé en cas d'erreur.
pub fn write_batches(
    path: impl AsRef<Path>,
    schema: SchemaRef,
    batches: &[RecordBatch],
    mut opts: ArrowWriteOptions,
) -> Result<WriteReport> {
    for (name, len) in infer_char_lengths(batches, opts.write.encoding) {
        opts.char_lengths.entry(name).or_insert(len);
    }
    let mut w = ArrowWriter::create(path, schema, opts)?;
    for b in batches {
        w.write(b)?;
    }
    w.commit()
}

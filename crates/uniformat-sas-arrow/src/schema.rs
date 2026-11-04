//! Schéma Arrow d'une table, calculé depuis les métadonnées seules (ADR 0003, décisions 1 à 3).

use std::collections::HashMap;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use uniformat_sas::temporal::{sas_datetime_to_parts, DateTimeParts};
use uniformat_sas::{
    encoding_for_code, Column, ColumnKind, Compression, InvalidBytes, LogicalType, TableMeta,
};

use crate::options::{DateMode, MissingMode, ReadOptions, TimeMode, TimestampUnit};

/// Représentation Arrow d'une colonne SAS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Repr {
    Float64,
    Date32,
    TimestampMicro,
    TimestampSecond,
    Time64Micro,
    DurationMicro,
    Utf8,
    Binary,
    SidecarU8,
}

/// Champ du schéma complet et d'où il vient.
#[derive(Debug, Clone)]
pub(crate) struct FieldPlan {
    pub(crate) field: Field,
    /// Rang de la colonne SAS dans `TableMeta::columns`.
    pub(crate) source: usize,
    pub(crate) repr: Repr,
}

fn repr_of(meta: &TableMeta, col: &Column, opts: &ReadOptions) -> Repr {
    match col.kind {
        ColumnKind::Char { .. } => {
            let decodable = opts.encoding_override.is_some()
                || encoding_for_code(meta.header.encoding_code).is_some();
            if opts.invalid_bytes == InvalidBytes::Binary || !decodable {
                Repr::Binary
            } else {
                Repr::Utf8
            }
        }
        ColumnKind::Numeric { .. } => {
            if opts.dates == DateMode::AsFloat {
                return Repr::Float64;
            }
            match col.logical {
                LogicalType::Date => Repr::Date32,
                LogicalType::DateTime => match opts.timestamp_unit {
                    TimestampUnit::Microsecond => Repr::TimestampMicro,
                    TimestampUnit::Second => Repr::TimestampSecond,
                },
                LogicalType::Time => match opts.time_mode {
                    TimeMode::Time64 => Repr::Time64Micro,
                    TimeMode::Duration => Repr::DurationMicro,
                },
                LogicalType::Number | LogicalType::Text => Repr::Float64,
            }
        }
    }
}

fn data_type(repr: Repr) -> DataType {
    match repr {
        Repr::Float64 => DataType::Float64,
        Repr::Date32 => DataType::Date32,
        Repr::TimestampMicro => DataType::Timestamp(TimeUnit::Microsecond, None),
        Repr::TimestampSecond => DataType::Timestamp(TimeUnit::Second, None),
        Repr::Time64Micro => DataType::Time64(TimeUnit::Microsecond),
        Repr::DurationMicro => DataType::Duration(TimeUnit::Microsecond),
        Repr::Utf8 => DataType::Utf8,
        Repr::Binary => DataType::Binary,
        Repr::SidecarU8 => DataType::UInt8,
    }
}

fn logical_name(l: LogicalType) -> &'static str {
    match l {
        LogicalType::Number => "number",
        LogicalType::Date => "date",
        LogicalType::DateTime => "datetime",
        LogicalType::Time => "time",
        LogicalType::Text => "text",
    }
}

fn field_metadata(col: &Column, sidecar: Option<&str>) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Some(label) = &col.label {
        m.insert("sas.label".to_owned(), label.clone());
    }
    if let Some(f) = &col.format {
        m.insert("sas.format".to_owned(), f.name.clone());
        m.insert("sas.format_width".to_owned(), f.width.to_string());
        m.insert("sas.format_decimals".to_owned(), f.decimals.to_string());
    }
    if let Some(f) = &col.informat {
        m.insert("sas.informat".to_owned(), f.name.clone());
    }
    m.insert("sas.length".to_owned(), col.width().to_string());
    m.insert(
        "sas.type".to_owned(),
        if col.kind.is_numeric() { "num" } else { "char" }.to_owned(),
    );
    m.insert(
        "sas.logical_type".to_owned(),
        logical_name(col.logical).to_owned(),
    );
    if col.indexed {
        m.insert("sas.index".to_owned(), "true".to_owned());
    }
    if let Some(name) = sidecar {
        m.insert("sas.missing_sidecar".to_owned(), name.to_owned());
        // Jamais déterminé sans lire les données (ADR 0003).
        m.insert("sas.has_special_missing".to_owned(), "unknown".to_owned());
    }
    m
}

/// Tous les champs du schéma complet (colonnes annexes comprises), dans l'ordre.
pub(crate) fn field_plans(meta: &TableMeta, opts: &ReadOptions) -> Vec<FieldPlan> {
    let mut out = Vec::with_capacity(meta.columns.len());
    for (i, col) in meta.columns.iter().enumerate() {
        let repr = repr_of(meta, col, opts);
        let sidecar_name = (opts.missing == MissingMode::Sidecar && col.kind.is_numeric())
            .then(|| format!("{}{}", col.name, opts.missing_suffix));
        let nullable = !matches!(repr, Repr::Utf8 | Repr::Binary) || opts.empty_as_null;
        let field = Field::new(&col.name, data_type(repr), nullable)
            .with_metadata(field_metadata(col, sidecar_name.as_deref()));
        out.push(FieldPlan {
            field,
            source: i,
            repr,
        });
        if let Some(name) = sidecar_name {
            out.push(FieldPlan {
                field: Field::new(name, DataType::UInt8, false),
                source: i,
                repr: Repr::SidecarU8,
            });
        }
    }
    out
}

/// Date ISO 8601 UTC d'un datetime SAS de l'en-tête (heure locale de l'écrivain moins son décalage UTC).
fn iso_utc(local_seconds: f64, tz_offset: f64) -> Option<String> {
    let utc = if tz_offset.is_finite() {
        local_seconds - tz_offset
    } else {
        local_seconds
    };
    let DateTimeParts {
        year,
        month,
        day,
        hour,
        minute,
        second,
    } = sas_datetime_to_parts(utc).ok()?;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn schema_metadata(meta: &TableMeta) -> HashMap<String, String> {
    let h = &meta.header;
    let mut m = HashMap::new();
    m.insert("sas.table_name".to_owned(), h.table_name.clone());
    m.insert("sas.encoding_code".to_owned(), h.encoding_code.to_string());
    if let Some(info) = uniformat_sas::encoding_info(h.encoding_code) {
        m.insert("sas.encoding".to_owned(), info.sas_name.to_owned());
    }
    if let Some(s) = iso_utc(h.created, h.created_tz_offset) {
        m.insert("sas.created".to_owned(), s);
    }
    if let Some(s) = iso_utc(h.modified, h.modified_tz_offset) {
        m.insert("sas.modified".to_owned(), s);
    }
    m.insert("sas.release".to_owned(), h.sas_release.clone());
    m.insert("sas.host".to_owned(), h.host.clone());
    m.insert(
        "sas.compression".to_owned(),
        match meta.compression {
            Compression::None => "none",
            Compression::Char => "char",
            Compression::Binary => "binary",
        }
        .to_owned(),
    );
    m.insert("sas.row_count".to_owned(), meta.row_count.to_string());
    m.insert(
        "sas.deleted_count".to_owned(),
        meta.deleted_count.to_string(),
    );
    m
}

/// Schéma Arrow complet d'une table, **sans lire de page de données** (ADR 0003).
///
/// Le schéma suit [`ReadOptions`] : les colonnes annexes `x__missing` n'existent qu'en
/// [`MissingMode::Sidecar`] ; les colonnes caractère sont `Binary` quand `invalid_bytes` est `Binary` ou quand
/// l'encodage de la table n'a pas d'équivalent standard (lecture en octets bruts).
#[must_use]
pub fn arrow_schema(meta: &TableMeta, opts: &ReadOptions) -> SchemaRef {
    let fields: Vec<Field> = field_plans(meta, opts)
        .into_iter()
        .map(|p| p.field)
        .collect();
    Arc::new(Schema::new_with_metadata(fields, schema_metadata(meta)))
}

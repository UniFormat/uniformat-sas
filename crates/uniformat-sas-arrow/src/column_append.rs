//! Shared temporal conversions, missing codes and text handling.
use crate::column_builder::{Builder, OutColumn};
use crate::{Error, OutOfRange, ReadOptions, ReadStats};
use uniformat_sas::temporal::{
    sas_date_to_unix_days, sas_datetime_to_unix_micros, sas_time_to_micros, ConvertError,
};
use uniformat_sas::{Column, MissingKind, NumValue, TextPolicy, TextValue};

/// Code de la colonne annexe (ADR 0003) : `0` présente, `1` `.`, `2` `._`, `3 + (lettre − 'A')`.
fn sidecar_code(v: NumValue) -> u8 {
    match v {
        NumValue::Value(_) => 0,
        NumValue::Missing(MissingKind::Dot) => 1,
        NumValue::Missing(MissingKind::Underscore) => 2,
        NumValue::Missing(MissingKind::Letter(c)) => 3u8.saturating_add(c.saturating_sub(b'A')),
    }
}

/// Valeur d'une conversion temporelle : `Ok(Some)`, `Ok(None)` (nul compté) ou l'erreur à rendre.
enum Converted<T> {
    Value(T),
    Null,
}

fn convert<T>(
    r: std::result::Result<T, ConvertError>,
    policy: OutOfRange,
    stats: &mut ReadStats,
    column: usize,
    row: u64,
    error: &mut Option<Error>,
) -> Converted<T> {
    match r {
        Ok(v) => Converted::Value(v),
        Err(_) => {
            stats.out_of_range = stats.out_of_range.saturating_add(1);
            if policy == OutOfRange::Error && error.is_none() {
                *error = Some(Error::OutOfRange { column, row });
            }
            Converted::Null
        }
    }
}

/// Ajoute la valeur de `row` pour la colonne de sortie `c`.
pub(crate) fn append<R: crate::row_values::ColumnRow>(
    c: &mut OutColumn,
    col: &Column,
    row: &R,
    opts: &ReadOptions,
    policy: &TextPolicy,
    stats: &mut ReadStats,
    error: &mut Option<Error>,
) {
    let at = row.physical_index();
    let number = if col.kind.is_numeric() {
        match row.number(col) {
            Ok(v) => v,
            Err(e) => {
                *error = Some(e);
                return;
            }
        }
    } else {
        NumValue::Missing(MissingKind::Dot)
    };
    match &mut c.builder {
        Builder::Sidecar(b) => b.append_value(sidecar_code(number)),
        Builder::Float64(b) => match number {
            NumValue::Value(v) => b.append_value(v),
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::Date32(b) => match number {
            NumValue::Value(v) => match convert(
                sas_date_to_unix_days(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(d) => b.append_value(d),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::TimestampMicro(b) => match number {
            NumValue::Value(v) => match convert(
                sas_datetime_to_unix_micros(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(t) => b.append_value(t),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::TimestampSecond(b) => match number {
            NumValue::Value(v) => match convert(
                sas_datetime_to_unix_micros(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(t) => b.append_value(t.div_euclid(1_000_000)),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::Time64Micro(b) => match number {
            NumValue::Value(v) => {
                // Time64 ne représente que 0 ≤ t < 86 400 s
                let r = sas_time_to_micros(v).and_then(|t| {
                    if (0..86_400_000_000).contains(&t) {
                        Ok(t)
                    } else {
                        Err(ConvertError::OutOfRange)
                    }
                });
                match convert(r, opts.out_of_range, stats, col.index, at, error) {
                    Converted::Value(t) => b.append_value(t),
                    Converted::Null => b.append_null(),
                }
            }
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::DurationMicro(b) => match number {
            NumValue::Value(v) => match convert(
                sas_time_to_micros(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(t) => b.append_value(t),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::Utf8(b) => match row.text_value(col, policy) {
            Ok(TextValue::Text { value, replaced }) => {
                if replaced {
                    stats.replaced_text = stats.replaced_text.saturating_add(1);
                }
                if opts.empty_as_null && value.is_empty() {
                    b.append_null();
                } else {
                    b.append_value(&value);
                }
            }
            Ok(TextValue::Bytes(_)) => b.append_null(),
            Err(e) => {
                if error.is_none() {
                    *error = Some(e);
                }
                b.append_null();
            }
        },
        Builder::Binary(b) => {
            let raw = match row.bytes(col, policy.trim_trailing) {
                Ok(b) => b,
                Err(e) => {
                    *error = Some(e);
                    return;
                }
            };
            if opts.empty_as_null && raw.is_empty() {
                b.append_null();
            } else {
                b.append_value(raw);
            }
        }
    }
}

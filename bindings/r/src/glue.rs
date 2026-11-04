//! Glue extendr : une seule fonction native, `sas7bdat_read_native`, qui rend une liste de colonnes et d'attributs.
//! Le `data.frame`, les conditions R (`sas7bdat_error`) et les avertissements sont construits côté R (`rpkg/R`).

use std::path::Path;

use extendr_api::prelude::*;

use crate::convert::{
    read_table, write_table, RClass, RColumn, RData, RReadOptions, RWriteColumn, RWriteOptions,
};

fn column_to_robj(c: RColumn) -> extendr_api::Result<Robj> {
    let mut obj: Robj = match c.data {
        RData::Real(v) => Doubles::from_values(v).into(),
        RData::Str(v) => Strings::from_values(v.into_iter().map(Option::unwrap_or_default)).into(),
    };
    if let Some(label) = c.label {
        obj.set_attrib("label", label)?;
    }
    if let Some(f) = c.format_sas {
        obj.set_attrib("format.sas", f)?;
    }
    if let Some(f) = c.informat_sas {
        obj.set_attrib("informat.sas", f)?;
    }
    match c.class {
        RClass::Plain => {}
        RClass::Date => {
            obj.set_class(["Date"])?;
        }
        RClass::DateTime => {
            obj.set_class(["POSIXct", "POSIXt"])?;
            obj.set_attrib("tzone", "UTC")?;
        }
        RClass::Time => {
            obj.set_class(["hms", "difftime"])?;
            obj.set_attrib("units", "secs")?;
        }
    }
    Ok(obj)
}

fn failure(class: &str, message: String) -> Robj {
    list!(ok = false, class = class, message = message).into()
}

/// Lecture native : `list(ok, columns, names, table_attrs, n_rows, warnings)` ou `list(ok = FALSE, class, message)`.
/// @export
#[extendr]
fn sas7bdat_read_native(
    path: &str,
    col_select: Robj,
    n_max: f64,
    encoding: Robj,
    trim_ws: bool,
    strict: bool,
) -> Robj {
    let opts = RReadOptions {
        col_select: col_select
            .as_str_vector()
            .map(|v| v.into_iter().map(str::to_owned).collect()),
        n_max: (n_max.is_finite() && n_max >= 0.0).then_some(n_max as usize),
        encoding: encoding.as_str().map(str::to_owned),
        trim_ws,
        strict,
    };
    let table = match read_table(Path::new(path), &opts) {
        Ok(t) => t,
        Err(e) => return failure(e.class(), e.to_string()),
    };
    let names: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
    let mut columns = Vec::with_capacity(table.columns.len());
    for c in table.columns {
        match column_to_robj(c) {
            Ok(o) => columns.push(o),
            Err(e) => return failure("sas7bdat_format_error", e.to_string()),
        }
    }
    let attr_names: Vec<String> = table.attrs.iter().map(|(k, _)| k.clone()).collect();
    let attr_values: Vec<String> = table.attrs.iter().map(|(_, v)| v.clone()).collect();
    list!(
        ok = true,
        columns = List::from_values(columns),
        names = names,
        attr_names = attr_names,
        attr_values = attr_values,
        n_rows = table.rows as i32,
        warnings = table.warnings
    )
    .into()
}

fn opt_string(v: &Strings, i: usize) -> Option<String> {
    v.elt(i).is_na().then_some(()).map_or_else(
        || Some(v.elt(i).as_ref().to_owned()).filter(|s| !s.is_empty()),
        |()| None,
    )
}

/// Écriture native : colonnes normalisées côté R (`double` / `character`), `classes` parmi `num`, `date`,
/// `datetime`, `time`, `char`. Rend `list(ok, rows, warnings)` ou `list(ok = FALSE, class, message)`.
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)]
fn sas7bdat_write_native(
    path: &str,
    names: Strings,
    columns: List,
    classes: Strings,
    labels: Strings,
    formats: Strings,
    informats: Strings,
    compression: &str,
    encoding: &str,
    page_length: i32,
    on_unrepresentable: &str,
) -> Robj {
    let mut cols = Vec::with_capacity(columns.len());
    for (i, (_, value)) in columns.iter().enumerate() {
        let data = if let Ok(d) = Doubles::try_from(value.clone()) {
            RData::Real(d.iter().map(|x| x.0).collect())
        } else if let Ok(s) = Strings::try_from(value.clone()) {
            RData::Str(
                s.iter()
                    .map(|x| (!x.is_na()).then(|| x.as_ref().to_owned()))
                    .collect(),
            )
        } else {
            return failure(
                "sas7bdat_usage_error",
                format!("colonne {i} : type non pris en charge"),
            );
        };
        let class = match classes.elt(i).as_ref() {
            "date" => RClass::Date,
            "datetime" => RClass::DateTime,
            "time" => RClass::Time,
            _ => RClass::Plain,
        };
        cols.push(RWriteColumn {
            name: names.elt(i).as_ref().to_owned(),
            data,
            class,
            label: opt_string(&labels, i),
            format_sas: opt_string(&formats, i),
            informat_sas: opt_string(&informats, i),
        });
    }
    let opts = RWriteOptions {
        compression: compression.to_owned(),
        encoding: encoding.to_owned(),
        page_length: u32::try_from(page_length).unwrap_or(65_536),
        on_unrepresentable: on_unrepresentable.to_owned(),
    };
    match write_table(Path::new(path), &cols, &opts) {
        Ok((rows, warnings)) => list!(ok = true, rows = rows as f64, warnings = warnings).into(),
        Err(e) => failure(e.class(), e.to_string()),
    }
}

extendr_module! {
    mod uniformatsas;
    fn sas7bdat_read_native;
    fn sas7bdat_write_native;
}

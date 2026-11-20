//! XPORT bindings without Arrow: strict text, tagged NA and bounded R vectors.
use crate::{
    convert::{
        class_of, convert_num, format_name, missing_of, write_schema, RClass, RColumn, RData,
        RError, RReadOptions, RTable, RWriteColumn,
    },
    read_budget::ReadBudget,
};
use std::path::Path;
use uniformat_sas::{
    write::{CellValue, WriteEncoding},
    xpt::{Row, TextEncoding, WriteOptions},
    ColumnKind, Limits,
};
use uniformat_sas_io::{FileSource, IoError, XptFile, XptOpenOptions, XptWriter};

/// Explicit member selector; indices are zero-based in Rust and one-based in R.
#[derive(Debug, Clone, Default)]
pub enum MemberSelection {
    /// First dataset, whose metadata is accessible without indexing observations.
    #[default]
    First,
    /// Zero-based member index.
    Index(usize),
    /// Case-insensitive name; reject ambiguous matches.
    Name(String),
}
/// Parse an explicit XPORT text encoding.
pub fn encoding(label: &str) -> Result<TextEncoding, RError> {
    match label.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Ok(TextEncoding::Utf8),
        "latin1" | "latin-1" | "iso-8859-1" => Ok(TextEncoding::Latin1),
        "wlatin1" | "windows-1252" | "cp1252" => Ok(TextEncoding::Windows1252),
        _ => Err(RError::Usage(
            "encoding must be UTF-8, latin1 or windows-1252".into(),
        )),
    }
}
fn io(e: IoError) -> RError {
    match e {
        IoError::Io(e) => RError::Io(e.to_string()),
        e => RError::Format(e.to_string()),
    }
}
fn format(e: uniformat_sas::Error) -> RError {
    RError::Format(e.to_string())
}
/// Read a selected XPORT member. n_max=0 on the first member reads only headers.
pub fn read_table(
    path: &Path,
    opts: &RReadOptions,
    selection: &MemberSelection,
) -> Result<RTable, RError> {
    let encoding = encoding(opts.encoding.as_deref().unwrap_or("UTF-8"))?;
    let file = XptFile::open(
        FileSource::open(path).map_err(|e| RError::Io(e.to_string()))?,
        XptOpenOptions {
            encoding,
            ..Default::default()
        },
    )
    .map_err(io)?;
    let first = matches!(
        selection,
        MemberSelection::First | MemberSelection::Index(0)
    );
    let member = if first && opts.n_max == Some(0) {
        None
    } else {
        let mut members = file.members().map_err(io)?;
        let index = match selection {
            MemberSelection::First => 0,
            MemberSelection::Index(i) => *i,
            MemberSelection::Name(n) => {
                let indices = members
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.metadata().header().name.eq_ignore_ascii_case(n))
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>();
                if indices.len() != 1 {
                    return Err(RError::Usage("XPT member absent or ambiguous".into()));
                }
                indices[0]
            }
        };
        if index >= members.len() {
            return Err(RError::Usage("XPT member index out of range".into()));
        }
        Some(members.swap_remove(index))
    };
    let meta = member.as_ref().map_or(file.metadata(), |m| m.metadata());
    let selected = match &opts.col_select {
        None => meta.columns().iter().collect::<Vec<_>>(),
        Some(names) => names
            .iter()
            .map(|n| {
                meta.columns()
                    .iter()
                    .find(|c| &c.name == n)
                    .ok_or_else(|| RError::Usage(format!("unknown column {n:?}")))
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    let budget = ReadBudget::new(
        &selected,
        member.as_ref().map_or(0, |m| m.row_count()),
        opts,
    )?;
    let mut data = selected
        .iter()
        .map(|c| match c.kind {
            ColumnKind::Numeric { .. } => RData::Real(Vec::new()),
            ColumnKind::Char { .. } => RData::Str(Vec::new()),
        })
        .collect::<Vec<_>>();
    let classes = selected.iter().map(|c| class_of(c)).collect::<Vec<_>>();
    let mut rows = 0;
    if let Some(member) = &member {
        for bytes in file.rows(member).take(opts.n_max.unwrap_or(usize::MAX)) {
            let bytes = bytes.map_err(io)?;
            let row = Row::new(&bytes, meta).map_err(format)?;
            budget.reserve_row(rows, &mut data)?;
            for ((c, data), class) in selected.iter().zip(&mut data).zip(&classes) {
                match data {
                    RData::Real(v) => {
                        v.push(convert_num(*class, row.number(c.index).map_err(format)?))
                    }
                    RData::Str(v) => {
                        let text = row.text(c.index, opts.trim_ws).map_err(format)?;
                        if text.contains('\0') {
                            return Err(RError::Format(
                                "XPT text contains NUL, not representable in R".into(),
                            ));
                        }
                        v.push(Some(text.into_owned()));
                    }
                }
            }
            rows += 1;
        }
    }
    let columns = selected
        .iter()
        .zip(data)
        .zip(classes)
        .map(|((c, data), class)| RColumn {
            name: c.name.clone(),
            data,
            class,
            label: c.label.clone(),
            format_sas: c.format.as_ref().map(format_name),
            informat_sas: c.informat.as_ref().map(format_name),
            width: Some(c.width() as u32),
        })
        .collect();
    Ok(RTable {
        columns,
        rows,
        warnings: Vec::new(),
        attrs: vec![
            ("label".into(), meta.header().label.clone()),
            ("xpt.name".into(), meta.header().name.clone()),
            ("xpt.created".into(), meta.header().created.clone()),
            ("xpt.modified".into(), meta.header().modified.clone()),
            (
                "xpt.version".into(),
                meta.header().version.number().to_string(),
            ),
        ],
    })
}
/// Write deterministic XPORT atomically; preserve haven tagged NA.
pub fn write_table(
    path: &Path,
    columns: &[RWriteColumn],
    opts: &WriteOptions,
) -> Result<u64, RError> {
    let enc = match opts.encoding {
        TextEncoding::Utf8 => WriteEncoding::Utf8,
        TextEncoding::Latin1 => WriteEncoding::Latin1,
        TextEncoding::Windows1252 => WriteEncoding::Wlatin1,
    };
    let rows = columns.first().map_or(0, |c| match &c.data {
        RData::Real(v) => v.len(),
        RData::Str(v) => v.len(),
    });
    if columns.iter().any(|c| match &c.data {
        RData::Real(v) => v.len() != rows,
        RData::Str(v) => v.len() != rows,
    }) {
        return Err(RError::Usage("XPT columns have different lengths".into()));
    }
    let mut opts = opts.clone();
    if opts.version == uniformat_sas::xpt::Version::V8 && opts.row_count.is_none() {
        opts.row_count = Some(rows as u64);
    }
    let schema = write_schema(columns, enc);
    let mut writer = XptWriter::create(path, &schema, &opts, &Limits::default()).map_err(io)?;
    for i in 0..rows {
        let cells = columns
            .iter()
            .map(|c| match &c.data {
                RData::Real(v) => {
                    let x = v[i];
                    if x.is_nan() {
                        CellValue::Missing(missing_of(x))
                    } else {
                        CellValue::Num(match c.class {
                            RClass::Date => x + 3653.0,
                            RClass::DateTime => x + 315_619_200.0,
                            _ => x,
                        })
                    }
                }
                RData::Str(v) => match &v[i] {
                    Some(s) => CellValue::Text(s),
                    None => CellValue::Null,
                },
            })
            .collect::<Vec<_>>();
        writer.write_row(&cells).map_err(io)?;
    }
    writer.commit().map_err(io)
}

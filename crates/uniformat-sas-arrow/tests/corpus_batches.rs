//! Critères de fin de J4 côté Arrow (`docs/roadmap.md`) : les lots Arrow redonnent les valeurs du lecteur Rust direct
//! sur tout le corpus, le schéma ne lit pas de page de données, `x__missing` redonne les 28 codes, les groupes de
//! pages (partitions) redonnent les mêmes lignes. Sauté sans `UNIFORMAT_CORPUS`.

use std::sync::Arc;

use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, DurationMicrosecondType, Float64Type, Time64MicrosecondType,
    TimestampMicrosecondType, UInt8Type,
};
use arrow_array::{Array, RecordBatch};
use arrow_schema::{DataType, TimeUnit};
use uniformat_sas::temporal::{
    sas_date_to_unix_days, sas_datetime_to_unix_micros, sas_time_to_micros,
};
use uniformat_sas::{Column, ColumnKind, LogicalType, NumValue, TextValue};
use uniformat_sas_arrow::{arrow_schema, BatchReader, MissingMode, ReadOptions};
use uniformat_sas_io::{ByteSource, CountingSource, FileSource, OpenOptions, PlanMode, SasFile};
use uniformat_testkit::Corpus;

/// Valeur attendue d'une cellule, calculée avec le lecteur direct.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    F64(Option<f64>),
    Date(Option<i32>),
    Ts(Option<i64>),
    Time(Option<i64>),
    Str(String),
    Bytes(Vec<u8>),
}

fn open_options_for(path: &std::path::Path) -> OpenOptions {
    // U22 : jeu de données B de datareader (test16 à test21), voir `corpus_read.rs`
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let dataset_b = stem
        .strip_prefix("test")
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| (16..=21).contains(&n));
    let mut o = OpenOptions::default();
    if path.to_string_lossy().contains("datareader") && dataset_b {
        o.encoding_override = Some(uniformat_sas::UTF_8);
    }
    o
}

fn read_options_for(open: &OpenOptions) -> ReadOptions {
    ReadOptions {
        encoding_override: open.encoding_override,
        ..ReadOptions::default()
    }
}

fn expected_rows(file: &SasFile<FileSource>) -> Vec<Vec<Cell>> {
    let meta = file.meta();
    let decodable = file.options().encoding_override.is_some()
        || uniformat_sas::encoding_for_code(meta.header.encoding_code).is_some();
    let policy = uniformat_sas::TextPolicy::default();
    let mut rows = Vec::new();
    file.for_each_row(|row| {
        let cells = meta
            .columns
            .iter()
            .map(|c| cell(c, &row, decodable, &policy))
            .collect();
        rows.push(cells);
    })
    .expect("lecture directe");
    rows
}

fn cell(
    c: &Column,
    row: &uniformat_sas::RowRef<'_>,
    decodable: bool,
    policy: &uniformat_sas::TextPolicy,
) -> Cell {
    match c.kind {
        ColumnKind::Char { .. } => {
            if decodable {
                match row.text_value(c, policy).expect("texte") {
                    TextValue::Text { value, .. } => Cell::Str(value.into_owned()),
                    TextValue::Bytes(b) => Cell::Bytes(b.to_vec()),
                }
            } else {
                Cell::Bytes(row.char_bytes(c).to_vec())
            }
        }
        ColumnKind::Numeric { .. } => {
            let v = match row.num(c) {
                NumValue::Value(v) => Some(v),
                NumValue::Missing(_) => None,
            };
            match c.logical {
                LogicalType::Date => Cell::Date(v.and_then(|v| sas_date_to_unix_days(v).ok())),
                LogicalType::DateTime => {
                    Cell::Ts(v.and_then(|v| sas_datetime_to_unix_micros(v).ok()))
                }
                LogicalType::Time => Cell::Time(
                    v.and_then(|v| sas_time_to_micros(v).ok())
                        .filter(|t| (0..86_400_000_000).contains(t)),
                ),
                _ => Cell::F64(v),
            }
        }
    }
}

fn check_batches(
    name: &str,
    batches: &[RecordBatch],
    expected: &[Vec<Cell>],
) -> Result<(), String> {
    let total: usize = batches.iter().map(RecordBatch::num_rows).sum();
    if total != expected.len() {
        return Err(format!(
            "{name} : {total} lignes Arrow, {} directes",
            expected.len()
        ));
    }
    let mut r = 0usize;
    for b in batches {
        for i in 0..b.num_rows() {
            let exp = &expected[r];
            for (j, want) in exp.iter().enumerate() {
                let col = b.column(j);
                let got = match (col.data_type(), want) {
                    (DataType::Float64, Cell::F64(_)) => Cell::F64(
                        (!col.is_null(i)).then(|| col.as_primitive::<Float64Type>().value(i)),
                    ),
                    (DataType::Date32, Cell::Date(_)) => Cell::Date(
                        (!col.is_null(i)).then(|| col.as_primitive::<Date32Type>().value(i)),
                    ),
                    (DataType::Timestamp(TimeUnit::Microsecond, None), Cell::Ts(_)) => Cell::Ts(
                        (!col.is_null(i))
                            .then(|| col.as_primitive::<TimestampMicrosecondType>().value(i)),
                    ),
                    (DataType::Time64(TimeUnit::Microsecond), Cell::Time(_)) => Cell::Time(
                        (!col.is_null(i))
                            .then(|| col.as_primitive::<Time64MicrosecondType>().value(i)),
                    ),
                    (DataType::Utf8, Cell::Str(_)) => {
                        Cell::Str(col.as_string::<i32>().value(i).to_owned())
                    }
                    (DataType::Binary, Cell::Bytes(_)) => {
                        Cell::Bytes(col.as_binary::<i32>().value(i).to_vec())
                    }
                    (t, w) => return Err(format!("{name} : type {t:?} pour {w:?}")),
                };
                let same = match (&got, want) {
                    (Cell::F64(a), Cell::F64(b)) => a == b,
                    _ => &got == want,
                };
                if !same {
                    return Err(format!(
                        "{name} : ligne {r} colonne {j} : {got:?} ≠ {want:?}"
                    ));
                }
            }
            r += 1;
        }
    }
    Ok(())
}

#[test]
fn batches_equal_direct_reader_on_the_whole_corpus() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let (mut files, mut cells) = (0usize, 0usize);
    let mut failures = Vec::new();
    for f in corpus.files().expect("corpus") {
        let open = open_options_for(&f.path);
        let ro = read_options_for(&open);
        let file = match SasFile::open(FileSource::open(&f.path).expect("source"), open) {
            Ok(file) => Arc::new(file),
            Err(_) => continue,
        };
        let expected = expected_rows(&file);
        let pages = 0..file.meta().header.page_count;
        let reader = BatchReader::new(Arc::clone(&file), pages, None, None, ro).expect("lecteur");
        let mut batches = Vec::new();
        let mut err = None;
        for b in reader {
            match b {
                Ok(b) => batches.push(b),
                Err(e) => {
                    err = Some(e.to_string());
                    break;
                }
            }
        }
        if let Some(e) = err {
            failures.push(format!("{} : {e}", f.path.display()));
            continue;
        }
        cells += expected.len() * file.meta().columns.len();
        files += 1;
        if let Err(e) = check_batches(&f.stem(), &batches, &expected) {
            failures.push(e);
        }
    }
    eprintln!("{files} fichiers, {cells} cellules comparées au lecteur direct");
    assert!(files >= 225, "{files} fichiers");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Critère 1 : le schéma d'un fichier de 5 000 colonnes se lit avec l'en-tête et les pages de métadonnées seulement.
#[test]
fn schema_of_5000_columns_reads_only_metadata() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let path = corpus
        .root()
        .join("files/taille/taille_wide5000_no.sas7bdat");
    let src = CountingSource::new(FileSource::open(&path).expect("source"));
    let file = SasFile::open(src, OpenOptions::default()).expect("open");
    let schema = arrow_schema(file.meta(), &ReadOptions::default());
    assert_eq!(
        schema.fields().len(),
        5001,
        "5 000 colonnes de données et la clé"
    );
    let h = &file.meta().header;
    let meta_pages = file
        .scanned_pages()
        .iter()
        .filter(|p| p.kind != uniformat_sas::PageKind::Data)
        .count() as u64;
    let bound = u64::from(h.header_length) + meta_pages * u64::from(h.page_length);
    let read = file.source().bytes_read();
    eprintln!(
        "5000 colonnes : {read} octets lus, borne HL + {meta_pages} × P = {bound}, fichier de {} octets",
        file.source().len()
    );
    assert!(
        meta_pages >= 2,
        "le fichier large doit avoir plusieurs pages de métadonnées"
    );
    assert!(read <= bound, "{read} octets lus > {bound}");
}

/// Critère 5 : `x__missing` redonne les 28 codes de `valeurs_manquants_no`.
#[test]
fn sidecar_gives_the_28_missing_codes() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let path = corpus
        .root()
        .join("files/valeurs/valeurs_manquants_no.sas7bdat");
    let file = Arc::new(
        SasFile::open(
            FileSource::open(&path).expect("source"),
            OpenOptions::default(),
        )
        .expect("open"),
    );
    let ro = ReadOptions {
        missing: MissingMode::Sidecar,
        ..ReadOptions::default()
    };
    let schema = arrow_schema(file.meta(), &ro);
    let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
    assert_eq!(names, ["x", "x__missing"]);
    assert_eq!(
        schema
            .field(0)
            .metadata()
            .get("sas.missing_sidecar")
            .map(String::as_str),
        Some("x__missing")
    );
    let pages = 0..file.meta().header.page_count;
    let batches: Vec<RecordBatch> = BatchReader::new(file, pages, None, None, ro)
        .expect("lecteur")
        .collect::<Result<_, _>>()
        .expect("lots");
    let mut codes = Vec::new();
    let mut nulls = 0;
    for b in &batches {
        let code = b.column(1).as_primitive::<UInt8Type>();
        for i in 0..b.num_rows() {
            codes.push(code.value(i));
            nulls += usize::from(b.column(0).is_null(i));
        }
    }
    assert_eq!(codes, (1..=28).collect::<Vec<u8>>());
    assert_eq!(nulls, 28, "x est nulle dès que x__missing ≠ 0");
}

/// Projection (dans l'ordre demandé, colonnes annexes comprises) et `LIMIT`.
#[test]
fn projection_and_limit_are_pushed_down() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let path = corpus.root().join("files/taille/taille_n50000_no.sas7bdat");
    let file = Arc::new(
        SasFile::open(
            FileSource::open(&path).expect("source"),
            OpenOptions::default(),
        )
        .expect("open"),
    );
    let pages = 0..file.meta().header.page_count;
    let ro = ReadOptions {
        batch_rows: 1000,
        ..ReadOptions::default()
    };
    let full = arrow_schema(file.meta(), &ro);
    let reader = BatchReader::new(
        Arc::clone(&file),
        pages.clone(),
        Some(vec![1, 0]),
        Some(2500),
        ro.clone(),
    )
    .expect("lecteur");
    let schema = reader.schema();
    assert_eq!(schema.field(0).name(), full.field(1).name());
    assert_eq!(schema.field(1).name(), full.field(0).name());
    let rows: usize = reader.map(|b| b.expect("lot").num_rows()).sum();
    assert_eq!(rows, 2500);
    // projection hors schéma
    assert!(BatchReader::new(
        Arc::clone(&file),
        pages.clone(),
        Some(vec![9999]),
        None,
        ro.clone()
    )
    .is_err());
    // projection vide : le compte de lignes est conservé
    let n: usize = BatchReader::new(file, pages, Some(vec![]), None, ro)
        .expect("lecteur")
        .map(|b| b.expect("lot").num_rows())
        .sum();
    assert_eq!(n, 50_000);
}

/// Les groupes de pages d'un plan (1, 4, 8 partitions) redonnent les lignes de la lecture séquentielle, dans l'ordre
/// des groupes, pour le plan rapide comme pour le plan complet.
#[test]
fn page_groups_reproduce_sequential_read() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    for name in [
        "taille/taille_n50000_no",
        "suppression/suppression_n100_del10_no",
        "taille/taille_buf4096_no",
    ] {
        let path = corpus.root().join(format!("files/{name}.sas7bdat"));
        let file = Arc::new(
            SasFile::open(
                FileSource::open(&path).expect("source"),
                OpenOptions::default(),
            )
            .expect("open"),
        );
        let ro = ReadOptions {
            batch_rows: 4096,
            ..ReadOptions::default()
        };
        let pages = 0..file.meta().header.page_count;
        let sequential: Vec<RecordBatch> =
            BatchReader::new(Arc::clone(&file), pages, None, None, ro.clone())
                .expect("lecteur")
                .collect::<Result<_, _>>()
                .expect("lots");
        let want = flatten_first_column(&sequential);
        for mode in [PlanMode::Fast, PlanMode::Full] {
            let plan = Arc::new(file.page_plan(mode).expect("plan"));
            for parts in [1usize, 4, 8] {
                let mut got = Vec::new();
                for g in plan.split(parts) {
                    let r = BatchReader::with_group(
                        Arc::clone(&file),
                        &g,
                        Arc::clone(&plan),
                        None,
                        None,
                        ro.clone(),
                    )
                    .expect("partition");
                    let batches: Vec<RecordBatch> = r.collect::<Result<_, _>>().expect("lots");
                    got.extend(flatten_first_column(&batches));
                }
                assert_eq!(got, want, "{name} {mode:?} {parts} partitions");
            }
        }
    }
}

fn flatten_first_column(batches: &[RecordBatch]) -> Vec<String> {
    let mut out = Vec::new();
    for b in batches {
        let col = b.column(0);
        for i in 0..b.num_rows() {
            out.push(match col.data_type() {
                DataType::Float64 => format!("{:?}", col.as_primitive::<Float64Type>().value(i)),
                DataType::Utf8 => col.as_string::<i32>().value(i).to_owned(),
                DataType::Duration(_) => {
                    format!("{}", col.as_primitive::<DurationMicrosecondType>().value(i))
                }
                other => format!("{other:?}{i}"),
            });
        }
    }
    out
}

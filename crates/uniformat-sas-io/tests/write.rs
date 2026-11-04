//! Écriture (J6) : relecture par le lecteur du cœur des fichiers produits par le driver.

use std::io::Cursor;

use uniformat_sas::write::{
    CellValue, Unrepresentable, WriteColumn, WriteEncoding, WriteOptions, WriteSchema,
};
use uniformat_sas::{ColumnKind, Compression, MissingKind, NumValue};
use uniformat_sas_io::{system_options, OpenOptions, SasFile, SasWriter, SliceSource};

fn write_vec(
    schema: &WriteSchema,
    opts: WriteOptions,
    rows: &[Vec<CellValue<'_>>],
) -> (Vec<u8>, uniformat_sas::write::WriteReport) {
    let mut w = SasWriter::new(Cursor::new(Vec::new()), schema, opts).expect("schéma");
    for r in rows {
        w.write_row(r).expect("ligne");
    }
    let (cur, report) = w.finish().expect("fin");
    (cur.into_inner(), report)
}

#[derive(Debug, PartialEq)]
enum V {
    N(f64),
    M(MissingKind),
    T(String),
}

fn read_back(bytes: &[u8]) -> (uniformat_sas::TableMeta, Vec<Vec<V>>) {
    let file = SasFile::open(SliceSource(bytes), OpenOptions::default()).expect("ouverture");
    let meta = file.meta().clone();
    let mut out = Vec::new();
    file.for_each_row(|row| {
        out.push(
            meta.columns
                .iter()
                .map(|c| match c.kind {
                    ColumnKind::Numeric { .. } => match row.num(c) {
                        NumValue::Value(v) => V::N(v),
                        NumValue::Missing(m) => V::M(m),
                    },
                    ColumnKind::Char { .. } => V::T(row.text(c).into_owned()),
                })
                .collect(),
        );
    })
    .expect("lecture");
    (meta, out)
}

fn schema() -> WriteSchema {
    WriteSchema {
        columns: vec![
            WriteColumn::numeric("i", 8),
            WriteColumn::numeric("x", 8)
                .with_label("Montant")
                .with_format("COMMA", 10, 2),
            WriteColumn::character("c", 5),
            WriteColumn::character("t", 20).with_label("texte"),
        ],
    }
}

fn rows(n: usize) -> Vec<Vec<CellValue<'static>>> {
    (1..=n)
        .map(|k| {
            vec![
                CellValue::Num(k as f64),
                CellValue::Num(k as f64 * 1.5),
                CellValue::Text("ab"),
                CellValue::Text("ligne"),
            ]
        })
        .collect()
}

#[test]
fn plain_roundtrip_over_several_pages_and_sizes() {
    for (n, page) in [
        (0usize, 65_536u32),
        (1, 65_536),
        (100, 4096),
        (3000, 65_536),
        (3000, 8192),
    ] {
        let opts = WriteOptions {
            page_length: page,
            ..system_options()
        };
        let (bytes, report) = write_vec(&schema(), opts, &rows(n));
        assert_eq!(report.rows, n as u64);
        let (meta, got) = read_back(&bytes);
        assert_eq!(meta.row_count, n as u64, "{n} lignes, page {page}");
        assert_eq!(meta.compression, Compression::None);
        assert_eq!(got.len(), n);
        for (k, r) in got.iter().enumerate() {
            let kf = (k + 1) as f64;
            assert_eq!(r[0], V::N(kf));
            assert_eq!(r[1], V::N(kf * 1.5));
            assert_eq!(r[2], V::T("ab".into()));
            assert_eq!(r[3], V::T("ligne".into()));
        }
        assert_eq!(meta.columns[1].label.as_deref(), Some("Montant"));
        assert_eq!(
            meta.columns[1]
                .format
                .as_ref()
                .map(|f| (f.name.as_str(), f.width, f.decimals)),
            Some(("COMMA", 10, 2))
        );
    }
}

#[test]
fn compressed_roundtrip_char_and_binary() {
    for algo in [Compression::Char, Compression::Binary] {
        for (n, page) in [
            (0usize, 65_536u32),
            (1, 65_536),
            (300, 4096),
            (10_000, 65_536),
            (70_000, 65_536),
        ] {
            let opts = WriteOptions {
                page_length: page,
                compression: algo,
                ..system_options()
            };
            let (bytes, _) = write_vec(&schema(), opts, &rows(n));
            let (meta, got) = read_back(&bytes);
            assert_eq!(meta.compression, algo, "{algo:?} {n}");
            assert_eq!(meta.row_count, n as u64);
            assert_eq!(got.len(), n);
            if let Some(last) = got.last() {
                assert_eq!(last[0], V::N(n as f64));
            }
        }
    }
}

#[test]
fn special_missing_values_and_encodings() {
    let schema = WriteSchema {
        columns: vec![
            WriteColumn::numeric("x", 8),
            WriteColumn::character("s", 12),
        ],
    };
    let mut cells: Vec<Vec<CellValue<'_>>> = vec![
        vec![CellValue::Missing(MissingKind::Dot), CellValue::Text("é")],
        vec![CellValue::Missing(MissingKind::Underscore), CellValue::Null],
    ];
    for c in b'A'..=b'Z' {
        cells.push(vec![
            CellValue::Missing(MissingKind::Letter(c)),
            CellValue::Text("z"),
        ]);
    }
    cells.push(vec![CellValue::Num(f64::NAN), CellValue::Text("n")]);
    for enc in [
        WriteEncoding::Utf8,
        WriteEncoding::Latin1,
        WriteEncoding::Wlatin1,
    ] {
        let opts = WriteOptions {
            encoding: enc,
            ..system_options()
        };
        let (bytes, _) = write_vec(&schema, opts, &cells);
        let (meta, got) = read_back(&bytes);
        assert_eq!(meta.header.encoding_code, enc.code());
        assert_eq!(got[0], vec![V::M(MissingKind::Dot), V::T("é".into())]);
        assert_eq!(got[1][0], V::M(MissingKind::Underscore));
        assert_eq!(got[2][0], V::M(MissingKind::Letter(b'A')));
        assert_eq!(got[27][0], V::M(MissingKind::Letter(b'Z')));
        assert_eq!(got[28][0], V::M(MissingKind::Dot), "NaN s'écrit .");
    }
}

#[test]
fn schema_limits_are_refused_before_the_first_byte() {
    use uniformat_sas::write::{SchemaError, WriteError};
    let bad: Vec<(WriteSchema, &str)> = vec![
        (
            WriteSchema {
                columns: vec![WriteColumn::numeric("n".repeat(33), 8)],
            },
            "name",
        ),
        (
            WriteSchema {
                columns: vec![WriteColumn::numeric("x", 8).with_label("L".repeat(257))],
            },
            "label",
        ),
        (
            WriteSchema {
                columns: vec![WriteColumn::character("c", 32_768)],
            },
            "char",
        ),
        (
            WriteSchema {
                columns: vec![WriteColumn::numeric("x", 2)],
            },
            "num",
        ),
        (
            WriteSchema {
                columns: vec![WriteColumn::numeric("x", 8), WriteColumn::numeric("x", 8)],
            },
            "dup",
        ),
        (WriteSchema::default(), "empty"),
    ];
    for (schema, what) in bad {
        let sink = Cursor::new(Vec::new());
        let err = SasWriter::new(sink, &schema, system_options()).expect_err(what);
        assert!(
            matches!(
                err,
                uniformat_sas_io::WriteIoError::Write(WriteError::Schema(_))
            ),
            "{what}: {err}"
        );
        let _ = SchemaError::NoColumns;
    }
    // trop de colonnes pour la page demandée, sans agrandissement
    let cols: Vec<_> = (0..700)
        .map(|k| WriteColumn::numeric(format!("v{k}"), 8))
        .collect();
    let schema = WriteSchema { columns: cols };
    let opts = WriteOptions {
        grow_page: false,
        ..system_options()
    };
    let err = SasWriter::new(Cursor::new(Vec::new()), &schema, opts).expect_err("trop de colonnes");
    assert!(err.to_string().contains("colonnes"), "{err}");
    // avec agrandissement : la page passe à 131 072
    let w = SasWriter::new(Cursor::new(Vec::new()), &schema, system_options()).expect("agrandie");
    assert_eq!(w.page_length(), 131_072);
}

#[test]
fn unrepresentable_values_follow_the_policy() {
    let schema = WriteSchema {
        columns: vec![WriteColumn::numeric("x", 4), WriteColumn::character("s", 3)],
    };
    // 1/3 ne tient pas dans 4 octets
    let third = [CellValue::Num(1.0 / 3.0), CellValue::Text("abc")];
    let mut w = SasWriter::new(Cursor::new(Vec::new()), &schema, system_options()).unwrap();
    assert!(w.write_row(&third).is_err());
    let long = [CellValue::Num(1.0), CellValue::Text("abcdef")];
    assert!(w.write_row(&long).is_err());
    let inf = [CellValue::Num(f64::INFINITY), CellValue::Text("a")];
    assert!(w.write_row(&inf).is_err());
    let euro = [CellValue::Num(1.0), CellValue::Text("€")];
    let latin = WriteOptions {
        encoding: WriteEncoding::Latin1,
        ..system_options()
    };
    let mut w1 = SasWriter::new(Cursor::new(Vec::new()), &schema, latin.clone()).unwrap();
    assert!(w1.write_row(&euro).is_err());
    for policy in [Unrepresentable::Replace, Unrepresentable::Truncate] {
        let opts = WriteOptions {
            on_unrepresentable: policy,
            ..system_options()
        };
        let mut w = SasWriter::new(Cursor::new(Vec::new()), &schema, opts).unwrap();
        w.write_row(&third).expect("mantisse tronquée acceptée");
        if policy == Unrepresentable::Truncate {
            w.write_row(&[CellValue::Num(1.0), CellValue::Text("aéb")])
                .expect("troncature");
            let r = w.report().unwrap().clone();
            assert_eq!(
                r.truncated_values, 1,
                "aéb : le b est coupé, pas la moitié de é"
            );
            w.write_row(&long).expect("tronqué");
            assert_eq!(w.report().unwrap().truncated_values, 2);
        }
    }
    let opts = WriteOptions {
        on_unrepresentable: Unrepresentable::Replace,
        encoding: WriteEncoding::Latin1,
        ..system_options()
    };
    let mut w = SasWriter::new(Cursor::new(Vec::new()), &schema, opts).unwrap();
    w.write_row(&euro).expect("remplacé");
    assert_eq!(w.report().unwrap().replaced_chars, 1);
    let (bytes, _) = w.finish().unwrap();
    let (_, got) = read_back(&bytes.into_inner());
    assert_eq!(got[0][1], V::T("?".into()));
}

#[test]
fn file_writing_is_transactional() {
    let dir = std::env::temp_dir().join(format!("sas7bdat-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("t.sas7bdat");
    // erreur en cours d'écriture : aucun fichier, aucun temporaire
    {
        let mut w = SasWriter::create(&dest, &schema(), system_options()).unwrap();
        w.write_row(&rows(1)[0]).unwrap();
        assert!(w.write_row(&[CellValue::Num(1.0)]).is_err());
    } // abandon
    assert!(!dest.exists());
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "temporaire supprimé"
    );
    // schéma invalide : ni fichier ni temporaire
    let bad = WriteSchema {
        columns: vec![WriteColumn::numeric("n".repeat(40), 8)],
    };
    assert!(SasWriter::create(&dest, &bad, system_options()).is_err());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    // succès : le fichier apparaît d'un coup
    let mut w = SasWriter::create(&dest, &schema(), system_options()).unwrap();
    for r in rows(10) {
        w.write_row(&r).unwrap();
    }
    let report = w.commit().unwrap();
    assert_eq!(report.rows, 10);
    let (meta, got) = read_back(&std::fs::read(&dest).unwrap());
    assert_eq!((meta.row_count, got.len()), (10, 10));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

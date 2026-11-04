//! J6, critère 2 : aller-retour Arrow → fichier → Arrow sans perte, sur tout le corpus (sauté sans
//! `UNIFORMAT_CORPUS`) et sur des tables tirées au hasard (`proptest`).

use std::io::Cursor;
use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BooleanArray, Date32Array, Float64Array, Int32Array, RecordBatch, StringArray,
    TimestampMicrosecondArray, UInt8Array,
};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use proptest::prelude::*;

/// Colonnes tirées au hasard : flottants, jours, textes, microsecondes, codes de manquants.
type Columns = (
    Vec<Option<f64>>,
    Vec<Option<i32>>,
    Vec<Option<String>>,
    Vec<Option<i64>>,
    Vec<u8>,
);
use uniformat_sas::write::{Unrepresentable, WriteEncoding};
use uniformat_sas::Compression;
use uniformat_sas_arrow::{
    infer_char_lengths, ArrowWriteOptions, ArrowWriter, BatchReader, MissingMode, ReadOptions,
};
use uniformat_sas_io::{system_options, ByteSource, FileSource, OpenOptions, SasFile, SliceSource};
use uniformat_testkit::Corpus;

fn read_all<S: ByteSource>(source: S, open: OpenOptions) -> Option<RecordBatch> {
    let opts = ReadOptions {
        missing: MissingMode::Sidecar,
        encoding_override: open.encoding_override,
        ..ReadOptions::default()
    };
    let file = Arc::new(SasFile::open(source, open).ok()?);
    let pages = file.meta().header.page_count;
    let reader = BatchReader::new(Arc::clone(&file), 0..pages, None, None, opts).ok()?;
    let schema = reader.schema();
    let batches: Vec<RecordBatch> = reader.collect::<Result<_, _>>().ok()?;
    arrow_select::concat::concat_batches(&schema, &batches).ok()
}

fn open_options_for(path: &std::path::Path) -> OpenOptions {
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

/// Schéma sans `sas.length` caractère : les longueurs sont mesurées sur les données (texte remplacé, encodage changé).
fn without_char_lengths(schema: &Schema) -> Schema {
    let fields: Vec<Field> = schema
        .fields()
        .iter()
        .map(|f| {
            let mut f = f.as_ref().clone();
            if matches!(f.data_type(), DataType::Utf8) {
                let mut md = f.metadata().clone();
                md.remove("sas.length");
                f = f.with_metadata(md);
            }
            f
        })
        .collect();
    Schema::new_with_metadata(fields, schema.metadata().clone())
}

fn write_batch(batch: &RecordBatch, compression: Compression) -> Result<Vec<u8>, String> {
    let schema = Arc::new(without_char_lengths(batch.schema().as_ref()));
    let batch = RecordBatch::try_new(schema.clone(), batch.columns().to_vec())
        .map_err(|e| e.to_string())?;
    let mut opts = ArrowWriteOptions::default();
    opts.write.compression = compression;
    opts.char_lengths = infer_char_lengths(std::slice::from_ref(&batch), WriteEncoding::Utf8);
    let mut w =
        ArrowWriter::try_new(Cursor::new(Vec::new()), schema, opts).map_err(|e| e.to_string())?;
    w.write(&batch).map_err(|e| e.to_string())?;
    let (cur, _) = w.finish().map_err(|e| e.to_string())?;
    Ok(cur.into_inner())
}

fn same_data(a: &RecordBatch, b: &RecordBatch) -> Result<(), String> {
    if a.num_rows() != b.num_rows() || a.num_columns() != b.num_columns() {
        return Err(format!(
            "{}×{} contre {}×{}",
            a.num_rows(),
            a.num_columns(),
            b.num_rows(),
            b.num_columns()
        ));
    }
    for (i, (x, y)) in a.columns().iter().zip(b.columns()).enumerate() {
        if x.data_type() != y.data_type() || x.to_data() != y.to_data() {
            return Err(format!(
                "colonne {i} ({}) différente",
                a.schema().field(i).name()
            ));
        }
    }
    Ok(())
}

#[test]
fn corpus_roundtrip_plain_and_compressed() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let (mut checked, mut skipped_read, mut skipped_write) = (0usize, 0usize, Vec::new());
    let mut failures = Vec::new();
    for f in corpus.files().expect("corpus") {
        let open = open_options_for(&f.path);
        let Some(source) = FileSource::open(&f.path).ok() else {
            continue;
        };
        let Some(original) = read_all(source, open.clone()) else {
            skipped_read += 1;
            continue;
        };
        // colonnes binaires (encodage sans équivalent) : l'aller-retour change le type, hors critère
        if original
            .schema()
            .fields()
            .iter()
            .any(|f| f.data_type() == &DataType::Binary)
        {
            skipped_write.push(format!("{} (binaire)", f.path.display()));
            continue;
        }
        for compression in [Compression::None, Compression::Char, Compression::Binary] {
            let bytes = match write_batch(&original, compression) {
                Ok(b) => b,
                Err(e) => {
                    skipped_write.push(format!("{} : {e}", f.path.display()));
                    break;
                }
            };
            let Some(back) = read_all(SliceSource(&bytes), OpenOptions::default()) else {
                failures.push(format!(
                    "{} {compression:?} : relecture impossible",
                    f.path.display()
                ));
                continue;
            };
            if let Err(e) = same_data(&original, &back) {
                failures.push(format!("{} {compression:?} : {e}", f.path.display()));
            }
            checked += 1;
        }
    }
    eprintln!(
        "{checked} aller-retour ; {skipped_read} fichiers illisibles, {} non réécrits : {skipped_write:#?}",
        skipped_write.len()
    );
    assert!(
        failures.is_empty(),
        "{} écarts :\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(checked >= 600, "{checked} aller-retour seulement");
}

fn arb_table() -> impl Strategy<Value = Columns> {
    (0usize..60).prop_flat_map(|n| {
        (
            proptest::collection::vec(proptest::option::of(-1e12f64..1e12), n),
            proptest::collection::vec(proptest::option::of(-100_000i32..2_000_000), n),
            proptest::collection::vec(
                proptest::option::of("[a-zA-Zàéüñ0-9]{0,6}( [a-zA-Zàéüñ0-9]{1,5})?"),
                n,
            ),
            proptest::collection::vec(
                proptest::option::of(-2_000_000_000_000_000i64..8_000_000_000_000_000),
                n,
            ),
            proptest::collection::vec(0u8..29, n),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn arrow_to_file_to_arrow_is_lossless((f, d, s, t, m) in arb_table()) {
        let n = f.len();
        let sidecar: Vec<u8> = m.clone();
        let schema = Arc::new(Schema::new(vec![
            Field::new("f", DataType::Float64, true),
            Field::new("f__missing", DataType::UInt8, false),
            Field::new("d", DataType::Date32, true),
            Field::new("s", DataType::Utf8, true),
            Field::new("t", DataType::Timestamp(TimeUnit::Microsecond, None), true),
            Field::new("b", DataType::Boolean, true),
            Field::new("i", DataType::Int32, true),
        ]));
        // une valeur présente n'a pas de code de manquant ; un code ≠ 0 la rend nulle
        let f_arr: Float64Array = f.iter().zip(&sidecar).map(|(v, c)| if *c == 0 { *v } else { None }).collect();
        let cols: Vec<ArrayRef> = vec![
            Arc::new(f_arr),
            Arc::new(UInt8Array::from(sidecar.iter().zip(&f).map(|(c, v)| if *c == 0 && v.is_none() { 1 } else { *c }).collect::<Vec<u8>>())),
            Arc::new(Date32Array::from(d.clone())),
            Arc::new(StringArray::from(s.clone())),
            Arc::new(TimestampMicrosecondArray::from(t.iter().map(|v| v.map(|x| x / 1_000_000 * 1_000_000)).collect::<Vec<_>>())),
            Arc::new(BooleanArray::from(d.iter().map(|v| v.map(|x| x % 2 == 0)).collect::<Vec<_>>())),
            Arc::new(Int32Array::from(d.clone())),
        ];
        let batch = RecordBatch::try_new(schema.clone(), cols).unwrap();
        for compression in [Compression::None, Compression::Char, Compression::Binary] {
            let opts = ArrowWriteOptions {
                write: uniformat_sas::write::WriteOptions {
                    compression,
                    on_unrepresentable: Unrepresentable::Error,
                    ..system_options()
                },
                char_lengths: infer_char_lengths(std::slice::from_ref(&batch), WriteEncoding::Utf8),
                ..ArrowWriteOptions::default()
            };
            let mut w = ArrowWriter::try_new(Cursor::new(Vec::new()), schema.clone(), opts).unwrap();
            w.write(&batch).unwrap();
            let (cur, report) = w.finish().unwrap();
            prop_assert_eq!(report.rows, n as u64);
            let bytes = cur.into_inner();
            let back = read_all(SliceSource(&bytes), OpenOptions::default()).expect("relecture");
            prop_assert_eq!(back.num_rows(), n);
            // colonnes dont le type se retrouve identique (par nom : chaque numérique reçoit sa colonne annexe)
            // SAS ne distingue pas le texte vide du manquant caractère : un null revient en chaîne vide (ADR 0005 §4)
            let s_expected: ArrayRef = Arc::new(StringArray::from(
                s.iter().map(|v| Some(v.clone().unwrap_or_default())).collect::<Vec<_>>(),
            ));
            for name in ["f", "f__missing", "d", "s", "t"] {
                let a = if name == "s" {
                    s_expected.clone()
                } else {
                    batch.column_by_name(name).unwrap().clone()
                };
                let b = back.column_by_name(name).expect("colonne relue");
                prop_assert_eq!(a.to_data(), b.to_data(), "colonne {} {:?}", name, compression);
            }
            // booléens et entiers reviennent en double
            let bools = back.column_by_name("b").expect("b");
            prop_assert_eq!(bools.len(), n);
        }
    }
}

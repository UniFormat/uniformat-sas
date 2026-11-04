//! Mandatory allocation regressions, independent of the external corpus and R runtime.
#[path = "../../../tests/support/fixtures.rs"]
mod fixtures;
use uniformat_sas::Compression;
use uniformat_sas_r::convert::{read_table, RData, RError, RReadOptions};

#[test]
fn forged_row_count_is_a_typed_error_and_n_max_bounds_the_import() {
    let dir = uniformat_testkit::tempdir().unwrap();
    let path = dir.path().join("forged.sas7bdat");
    let mut bytes = fixtures::numeric(1);
    fixtures::set_declared_rows(&mut bytes, 1, u64::MAX);
    std::fs::write(&path, bytes).unwrap();
    let result = std::panic::catch_unwind(|| read_table(&path, &RReadOptions::default()));
    assert!(matches!(result.unwrap(), Err(RError::ResourceLimit(_))));
    let opts = RReadOptions {
        n_max: Some(1),
        ..RReadOptions::default()
    };
    let table = read_table(&path, &opts).unwrap();
    assert_eq!(table.rows, 1);
    assert_eq!(table.columns[0].data, RData::Real(vec![0.0]));
}

#[test]
fn a_small_budget_refuses_wide_outputs_and_metadata_only_still_works() {
    let dir = uniformat_testkit::tempdir().unwrap();
    let path = dir.path().join("table.sas7bdat");
    std::fs::write(&path, fixtures::table(100, Compression::None)).unwrap();
    let opts = RReadOptions {
        max_bytes: 1024,
        ..RReadOptions::default()
    };
    assert!(matches!(
        read_table(&path, &opts),
        Err(RError::ResourceLimit(_))
    ));
    let meta = read_table(
        &path,
        &RReadOptions {
            n_max: Some(0),
            ..opts
        },
    )
    .unwrap();
    assert_eq!(meta.rows, 0);
    assert_eq!(meta.columns.len(), 2);
}

#[test]
fn synthetic_text_and_numeric_values_survive_all_compressions() {
    let dir = uniformat_testkit::tempdir().unwrap();
    for compression in [Compression::None, Compression::Char, Compression::Binary] {
        let path = dir.path().join("table.sas7bdat");
        std::fs::write(&path, fixtures::table(40, compression)).unwrap();
        let table = read_table(&path, &RReadOptions::default()).unwrap();
        assert_eq!(table.rows, 40);
        assert_eq!(
            table.columns[0].data,
            RData::Real((0..40).map(f64::from).collect())
        );
        assert_eq!(
            table.columns[1].data,
            RData::Str(vec![Some("texte synthétique".into()); 40])
        );
    }
}

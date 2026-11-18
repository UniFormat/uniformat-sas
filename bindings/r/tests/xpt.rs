//! Direct R conversion of XPORT, independently of an installed R interpreter.
use uniformat_sas::xpt::WriteOptions;
use uniformat_sas_r::{
    convert::{na_real, na_tag, tagged_na, RClass, RData, RReadOptions, RWriteColumn},
    xpt,
};
use uniformat_testkit::tempdir;
#[test]
fn tagged_missing_dates_and_strict_encoding_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.xpt");
    let numbers = vec![
        1.5,
        na_real(),
        tagged_na(b'_'),
        tagged_na(b'a'),
        tagged_na(b'z'),
    ];
    let column = |name: &str, data, class| RWriteColumn {
        name: name.into(),
        data,
        class,
        label: Some("Label".into()),
        format_sas: None,
        informat_sas: None,
    };
    let columns = vec![
        column("N", RData::Real(numbers.clone()), RClass::Plain),
        column("DAY", RData::Real(vec![-3653.; 5]), RClass::Date),
        column(
            "TEXT",
            RData::Str(vec![Some("été".into()); 5]),
            RClass::Plain,
        ),
    ];
    assert_eq!(
        xpt::write_table(&path, &columns, &WriteOptions::default()).unwrap(),
        5
    );
    let b = xpt::read_table(
        &path,
        &RReadOptions::default(),
        &xpt::MemberSelection::First,
    )
    .unwrap();
    assert_eq!(b.rows, 5);
    let RData::Real(n) = &b.columns[0].data else {
        panic!()
    };
    assert_eq!(n[0], 1.5);
    assert_eq!(
        n.iter().map(|&x| na_tag(x)).collect::<Vec<_>>(),
        numbers.iter().map(|&x| na_tag(x)).collect::<Vec<_>>()
    );
    assert_eq!(b.columns[1].data, columns[1].data);
    assert_eq!(b.columns[2].data, columns[2].data);
    let opts = RReadOptions {
        max_bytes: 1,
        ..Default::default()
    };
    assert!(xpt::read_table(&path, &opts, &Default::default()).is_err());
}

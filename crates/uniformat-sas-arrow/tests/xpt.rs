//! XPORT integration, preservation and resource-bound regressions.
use arrow_array::{
    Array, ArrayRef, BooleanArray, Date32Array, Float64Array, Int64Array, RecordBatch, StringArray,
    TimestampMicrosecondArray, UInt8Array,
};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use std::sync::Arc;
use uniformat_sas::xpt::TextEncoding;
use uniformat_sas_arrow::{
    xpt::{
        write_xpt_batches, xpt_schema, XptArrowWriter, XptBatchReader, XptReadOptions,
        XptWriteOptions,
    },
    MissingMode,
};
use uniformat_sas_io::{XptFile, XptOpenOptions};
use uniformat_testkit::tempdir;

fn input() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("N", DataType::Float64, true),
        Field::new("N__missing", DataType::UInt8, false),
        Field::new("TEXT", DataType::Utf8, false).with_metadata(
            [
                ("sas.length".into(), "12".into()),
                ("sas.label".into(), "Libellé".into()),
            ]
            .into(),
        ),
        Field::new("DAY", DataType::Date32, false),
        Field::new(
            "STAMP",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            false,
        ),
        Field::new("FLAG", DataType::Boolean, false),
        Field::new("INT", DataType::Int64, false),
    ]));
    let a: Vec<ArrayRef> = vec![
        Arc::new(Float64Array::from(
            (0..29)
                .map(|i| if i == 0 { Some(1.25) } else { None })
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt8Array::from((0..29).collect::<Vec<_>>())),
        Arc::new(StringArray::from(vec!["été"; 29])),
        Arc::new(Date32Array::from(vec![-3653; 29])),
        Arc::new(TimestampMicrosecondArray::from(vec![-1_000_000; 29])),
        Arc::new(BooleanArray::from(vec![true; 29])),
        Arc::new(Int64Array::from(vec![42; 29])),
    ];
    RecordBatch::try_new(schema, a).unwrap()
}
fn decode(data: Vec<u8>, mut opts: XptReadOptions) -> RecordBatch {
    opts.read.missing = MissingMode::Sidecar;
    let file = XptFile::open(data, opts.open.clone()).unwrap();
    let reader = XptBatchReader::new(file, opts, None, None).unwrap();
    let schema = reader.schema();
    let batches = reader.collect::<Result<Vec<_>, _>>().unwrap();
    arrow_select::concat::concat_batches(&schema, &batches).unwrap()
}
fn encode(batch: &RecordBatch, opts: XptWriteOptions) -> Vec<u8> {
    let mut w = XptArrowWriter::try_new(Vec::new(), batch.schema(), opts).unwrap();
    w.write(batch).unwrap();
    w.finish().unwrap().0
}
#[test]
fn arrow_roundtrip_preserves_all_missing_codes_dates_text_and_metadata() {
    let bytes = encode(&input(), XptWriteOptions::default());
    let mut opts = XptReadOptions::default();
    opts.read.batch_rows = 3;
    let b = decode(bytes.clone(), opts);
    let codes = b
        .column_by_name("N__missing")
        .unwrap()
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(codes.values().as_ref(), &(0u8..29).collect::<Vec<_>>());
    assert_eq!(
        b.column_by_name("DAY")
            .unwrap()
            .as_any()
            .downcast_ref::<Date32Array>()
            .unwrap()
            .value(0),
        -3653
    );
    assert_eq!(
        b.column_by_name("STAMP")
            .unwrap()
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .unwrap()
            .value(0),
        -1_000_000
    );
    assert_eq!(
        b.column_by_name("FLAG")
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .value(0),
        1.0
    );
    assert_eq!(
        b.schema().field_with_name("TEXT").unwrap().metadata()["sas.label"],
        "Libellé"
    );
    assert_eq!(encode(&b, XptWriteOptions::default()), bytes);
}
#[test]
fn explicit_encodings_distinguish_latin1_and_windows1252() {
    for (encoding, text) in [
        (TextEncoding::Utf8, "€été"),
        (TextEncoding::Latin1, "\u{80}été"),
        (TextEncoding::Windows1252, "€été"),
    ] {
        let b = RecordBatch::try_from_iter(vec![(
            "TEXT",
            Arc::new(StringArray::from(vec![text])) as ArrayRef,
        )])
        .unwrap();
        let mut wo = XptWriteOptions::default();
        wo.write.encoding = encoding;
        wo.char_lengths.insert("TEXT".into(), 12);
        let bytes = encode(&b, wo);
        let mut ro = XptReadOptions::default();
        ro.open.encoding = encoding;
        let back = decode(bytes, ro);
        assert_eq!(
            back.column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            text
        );
    }
}
#[test]
fn projection_limit_and_budget_are_enforced() {
    let bytes = encode(&input(), XptWriteOptions::default());
    let mut opts = XptReadOptions::default();
    opts.read.batch_rows = usize::MAX;
    opts.read.max_batch_bytes = 4096;
    let file = XptFile::open(bytes.clone(), XptOpenOptions::default()).unwrap();
    let b = XptBatchReader::new(file, opts.clone(), Some(vec![2]), Some(7))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(b.iter().map(|b| b.num_rows()).sum::<usize>(), 7);
    assert!(b
        .iter()
        .all(|b| b.num_columns() == 1 && b.get_array_memory_size() < 4096));
    opts.read.max_batch_bytes = 1;
    assert!(XptBatchReader::new(
        XptFile::open(bytes, XptOpenOptions::default()).unwrap(),
        opts,
        None,
        None
    )
    .is_err());
}
#[test]
fn strict_invalid_text_fails_once_and_binary_preserves_bytes() {
    let b = RecordBatch::try_from_iter(vec![(
        "TEXT",
        Arc::new(arrow_array::BinaryArray::from(vec![&b"\xff"[..]])) as ArrayRef,
    )])
    .unwrap();
    let mut wo = XptWriteOptions::default();
    wo.char_lengths.insert("TEXT".into(), 8);
    let bytes = encode(&b, wo);
    let mut reader = XptBatchReader::new(
        XptFile::open(bytes.clone(), Default::default()).unwrap(),
        Default::default(),
        None,
        None,
    )
    .unwrap();
    assert!(reader.next().unwrap().is_err());
    assert!(reader.next().is_none());
    let mut opts = XptReadOptions::default();
    opts.read.invalid_bytes = uniformat_sas::InvalidBytes::Binary;
    let back = decode(bytes, opts);
    assert_eq!(
        back.column(0)
            .as_any()
            .downcast_ref::<arrow_array::BinaryArray>()
            .unwrap()
            .value(0),
        b"\xff"
    );
}
#[test]
fn failed_batch_keeps_existing_destination_and_poison_prevents_commit() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("existing.xpt");
    std::fs::write(&path, b"unchanged").unwrap();
    let b = RecordBatch::try_from_iter(vec![(
        "N",
        Arc::new(Int64Array::from(vec![1, (1i64 << 53) + 1])) as ArrayRef,
    )])
    .unwrap();
    let mut writer = XptArrowWriter::create(&path, b.schema(), Default::default()).unwrap();
    assert!(writer.write(&b).is_err());
    assert!(writer.commit().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"unchanged");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn invalid_sidecars_and_unbounded_width_inference_are_rejected() {
    let b = RecordBatch::try_from_iter(vec![
        ("N", Arc::new(Float64Array::from(vec![None])) as ArrayRef),
        (
            "N__missing",
            Arc::new(UInt8Array::from(vec![29])) as ArrayRef,
        ),
    ])
    .unwrap();
    let mut w = XptArrowWriter::try_new(Vec::new(), b.schema(), Default::default()).unwrap();
    assert!(w.write(&b).is_err());
    assert!(w.finish().is_err());
    let dir = tempdir().unwrap();
    let path = dir.path().join("none.xpt");
    let b = RecordBatch::try_from_iter(vec![(
        "TEXT",
        Arc::new(StringArray::from(vec!["abc"])) as ArrayRef,
    )])
    .unwrap();
    let opts = XptWriteOptions {
        max_buffer_bytes: 1,
        ..Default::default()
    };
    assert!(write_xpt_batches(&path, b.schema(), [Ok(b)], opts).is_err());
    assert!(!path.exists());
}
#[test]
fn registry_conversion_sas7bdat_xpt_sas7bdat_preserves_values() {
    use uniformat_core::{convert, ReadRequest, Registry, WriteRequest};
    let dir = tempdir().unwrap();
    let src = dir.path().join("first.xpt");
    let sas = dir.path().join("copy.sas7bdat");
    let dst = dir.path().join("last.XPT");
    std::fs::write(&src, encode(&input(), Default::default())).unwrap();
    let mut r = Registry::new();
    uniformat_sas_arrow::register(&mut r);
    let req = ReadRequest {
        options: [("missing".into(), "sidecar".into())].into(),
        ..Default::default()
    };
    let info = r.by_name("xpt").unwrap().info(&src, &req).unwrap();
    assert_eq!(info.row_count, None);
    assert_eq!(
        convert(&r, &src, &sas, None, None, &req, &WriteRequest::default())
            .unwrap()
            .rows,
        29
    );
    assert_eq!(
        convert(&r, &sas, &dst, None, None, &req, &WriteRequest::default())
            .unwrap()
            .rows,
        29
    );
    let a = decode(std::fs::read(src).unwrap(), Default::default());
    let b = decode(std::fs::read(dst).unwrap(), Default::default());
    assert_eq!(a.columns(), b.columns());
}

#[test]
fn registry_v8_options_preserve_long_names_through_sas7bdat() {
    use uniformat_core::{convert, ReadRequest, Registry, WriteRequest};
    let dir = tempdir().unwrap();
    let src = dir.path().join("long.xpt");
    let sas = dir.path().join("long.sas7bdat");
    let dst = dir.path().join("back.xpt");
    let b = RecordBatch::try_from_iter(vec![
        (
            "long_numeric_name",
            Arc::new(Float64Array::from(vec![1.25, -2.5])) as ArrayRef,
        ),
        (
            "long_character_name",
            Arc::new(StringArray::from(vec!["x".repeat(250), "été".into()])) as ArrayRef,
        ),
    ])
    .unwrap();
    let request = WriteRequest {
        options: [("version".into(), "8".into())].into(),
    };
    let opts = uniformat_sas_arrow::xpt::write_options(&request.options).unwrap();
    write_xpt_batches(&src, b.schema(), [Ok(b.clone())], opts).unwrap();
    let mut registry = Registry::new();
    uniformat_sas_arrow::register(&mut registry);
    convert(
        &registry,
        &src,
        &sas,
        None,
        None,
        &ReadRequest::default(),
        &WriteRequest::default(),
    )
    .unwrap();
    assert!(convert(
        &registry,
        &sas,
        &dst,
        None,
        None,
        &ReadRequest::default(),
        &WriteRequest::default()
    )
    .is_err());
    assert!(!dst.exists());
    assert_eq!(
        convert(
            &registry,
            &sas,
            &dst,
            None,
            None,
            &ReadRequest::default(),
            &request
        )
        .unwrap()
        .rows,
        2
    );
    let info = registry
        .by_name("xpt")
        .unwrap()
        .info(&dst, &ReadRequest::default())
        .unwrap();
    assert_eq!(info.schema.metadata()["xpt.version"], "8");
    let mut opts = XptReadOptions::default();
    opts.read.batch_rows = 1;
    let reader = XptBatchReader::new(
        XptFile::open(std::fs::read(dst).unwrap(), Default::default()).unwrap(),
        opts,
        None,
        None,
    )
    .unwrap();
    let batches = reader.collect::<Result<Vec<_>, _>>().unwrap();
    let back = arrow_select::concat::concat_batches(&batches[0].schema(), &batches).unwrap();
    assert_eq!(back.columns(), b.columns());
}
#[test]
fn metadata_schema_does_not_need_observations() {
    let data = include_bytes!("../../../tests/data/xpt/v5-synthetic.xpt");
    let file = XptFile::open(data.to_vec(), Default::default()).unwrap();
    let schema = xpt_schema(file.metadata(), &Default::default()).unwrap();
    assert_eq!(schema.fields().len(), 2);
    assert_eq!(schema.metadata()["xpt.version"], "5");
}

#[test]
fn temporal_precision_loss_is_rejected_before_commit() {
    for array in [
        Arc::new(arrow_array::Date64Array::from(vec![1])) as ArrayRef,
        Arc::new(arrow_array::TimestampNanosecondArray::from(vec![1])) as ArrayRef,
        Arc::new(arrow_array::Time64NanosecondArray::from(vec![1])) as ArrayRef,
    ] {
        let batch = RecordBatch::try_from_iter(vec![("VALUE", array)]).unwrap();
        let mut writer =
            XptArrowWriter::try_new(Vec::new(), batch.schema(), Default::default()).unwrap();
        assert!(writer.write(&batch).is_err());
        assert!(writer.finish().is_err());
    }
    let batch = RecordBatch::try_from_iter(vec![(
        "VALUE",
        Arc::new(arrow_array::TimestampNanosecondArray::from(vec![
            1_000_000_000,
        ])) as ArrayRef,
    )])
    .unwrap();
    let back = decode(encode(&batch, Default::default()), Default::default());
    assert_eq!(
        back.column(0)
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .unwrap()
            .value(0),
        1_000_000
    );
}

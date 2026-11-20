//! V8 semantics, malicious metadata, streaming counts and atomicity.
use std::{borrow::Cow, io};
use uniformat_sas::{
    write::{CellValue, WriteColumn, WriteSchema},
    xpt::{Encoder, Row, Version, WriteOptions},
    Limits, SasFormat,
};
use uniformat_sas_io::{ByteSource, SliceSource, XptFile, XptOpenOptions, XptWriter};

fn options(count: Option<u64>) -> WriteOptions {
    WriteOptions {
        version: Version::V8,
        row_count: count,
        table_name: "LONG_DATASET_NAME".into(),
        ..Default::default()
    }
}
fn schema(extended: bool) -> WriteSchema {
    let mut first = WriteColumn::numeric("same_prefix_first", 8).with_label(format!(
        "{}{}",
        "a".repeat(39),
        "é".repeat(40)
    ));
    if extended {
        first.format = Some(SasFormat {
            name: "LONG_FORMAT_NAME".into(),
            width: 12,
            decimals: 2,
        });
    }
    WriteSchema {
        columns: vec![
            first,
            WriteColumn::character("same_prefix_second", 250).with_label("b".repeat(256)),
        ],
    }
}
fn fixture(extended: bool, count: Option<u64>) -> Vec<u8> {
    let mut writer = XptWriter::new(
        Vec::new(),
        &schema(extended),
        &options(count),
        &Limits::default(),
    )
    .unwrap();
    writer
        .write_row(&[CellValue::Num(1.25), CellValue::Text(&"x".repeat(250))])
        .unwrap();
    writer.finish().unwrap().0
}
fn marker(data: &[u8], name: &str) -> usize {
    data.as_chunks::<80>()
        .0
        .iter()
        .position(|c| &c[20..28] == format!("{name:<8}").as_bytes())
        .unwrap()
        * 80
}
fn opens(data: &[u8]) -> bool {
    XptFile::open(SliceSource(data), XptOpenOptions::default()).is_ok()
}

#[test]
fn both_label_layouts_preserve_full_names_utf8_labels_and_formats() {
    for extended in [false, true] {
        let bytes = fixture(extended, Some(1));
        let file = XptFile::open(bytes, XptOpenOptions::default()).unwrap();
        let meta = file.metadata();
        assert_eq!(meta.header().version, Version::V8);
        assert_eq!(meta.header().name, "LONG_DATASET_NAME");
        assert_eq!(meta.declared_row_count(), Some(1));
        for (got, expected) in meta.columns().iter().zip(schema(extended).columns) {
            assert_eq!(got.name, expected.name);
            assert_eq!(got.label.as_deref(), Some(expected.label.as_str()));
            assert_eq!(got.format, expected.format);
        }
        let members = file.members().unwrap();
        assert_eq!(members[0].row_count(), 1);
        let bytes = file.rows(&members[0]).next().unwrap().unwrap();
        assert_eq!(
            Row::new(&bytes, meta).unwrap().text(1, true).unwrap(),
            "x".repeat(250)
        );
    }
}

#[test]
fn counted_blank_rows_and_embedded_member_signatures_are_data() {
    let ws = WriteSchema {
        columns: vec![WriteColumn::character("TEXT", 1)],
    };
    for count in [0, 1, 79, 80, 81] {
        let mut writer =
            XptWriter::new(Vec::new(), &ws, &options(Some(count)), &Limits::default()).unwrap();
        for _ in 0..count {
            writer.write_row(&[CellValue::Null]).unwrap();
        }
        let file = XptFile::open(writer.finish().unwrap().0, XptOpenOptions::default()).unwrap();
        assert_eq!(file.members().unwrap()[0].row_count(), count);
    }
    let payload = fixture(false, Some(1))[240..400].to_vec();
    let ws = WriteSchema {
        columns: vec![WriteColumn::character("TEXT", 160)],
    };
    let mut writer =
        XptWriter::new(Vec::new(), &ws, &options(Some(1)), &Limits::default()).unwrap();
    writer.write_row(&[CellValue::Bytes(&payload)]).unwrap();
    let file = XptFile::open(writer.finish().unwrap().0, XptOpenOptions::default()).unwrap();
    let members = file.members().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(
        file.rows(&members[0]).next().unwrap().unwrap().as_ref(),
        payload
    );
    let mut legacy = XptWriter::new(Vec::new(), &ws, &options(None), &Limits::default()).unwrap();
    assert!(legacy.write_row(&[CellValue::Bytes(&payload)]).is_err());
}

#[test]
fn header_only_open_and_counted_index_do_not_scan_observations() {
    struct MetadataOnly {
        bytes: Vec<u8>,
        header_end: usize,
        data_end: usize,
    }
    impl ByteSource for MetadataOnly {
        fn len(&self) -> u64 {
            self.bytes.len() as u64
        }
        fn read_range(&self, at: u64, len: usize) -> io::Result<Cow<'_, [u8]>> {
            assert!(at as usize + len <= self.header_end || at as usize >= self.data_end);
            self.bytes.read_range(at, len)
        }
    }
    let bytes = fixture(true, Some(1));
    let header_end = marker(&bytes, "OBSV8") + 80;
    let source = MetadataOnly {
        bytes,
        header_end,
        data_end: header_end + 258,
    };
    let file = XptFile::open(source, XptOpenOptions::default()).unwrap();
    assert_eq!(file.members().unwrap()[0].row_count(), 1);
}

#[test]
fn legacy_count_variants_and_mixed_member_counts() {
    let mut data = fixture(false, None);
    let at = marker(&data, "OBSV8");
    for suffix in [
        [b' '; 32],
        *b"              0                 ",
        *b"000000000000000000000000000000  ",
    ] {
        let mut bytes = data.clone();
        bytes[at + 48..at + 80].copy_from_slice(&suffix);
        assert_eq!(
            XptFile::open(bytes, XptOpenOptions::default())
                .unwrap()
                .members()
                .unwrap()[0]
                .row_count(),
            1
        );
    }
    data.extend_from_slice(&fixture(true, Some(1))[240..]);
    let f = XptFile::open(data, XptOpenOptions::default()).unwrap();
    assert_eq!(
        f.members()
            .unwrap()
            .iter()
            .map(|m| m.row_count())
            .collect::<Vec<_>>(),
        vec![1, 1]
    );
}

#[test]
fn truncated_oversized_and_inconsistent_label_sections_are_rejected() {
    let data = fixture(true, Some(1));
    let label = marker(&data, "LABELV9");
    let obs = marker(&data, "OBSV8");
    for end in (0..=obs).step_by(80) {
        assert!(!opens(&data[..end]));
    }
    for (at, bytes) in [
        (label + 48, b"99999".to_vec()),
        (label + 80, vec![0, 0]),
        (label + 82, vec![0, 33]),
        (label + 84, vec![1, 1]),
        (label + 86, vec![255, 255]),
        (label + 90, vec![b'X']),
        (640 + 120, vec![0, 0]),
        (obs - 1, vec![0]),
        (obs + 48, b"             -1".to_vec()),
        (obs + 63, vec![b'1']),
        (240 + 20, b"MEMBER  ".to_vec()),
    ] {
        let mut bad = data.clone();
        bad[at..at + bytes.len()].copy_from_slice(&bytes);
        assert!(!opens(&bad), "accepted malformed field at {at}");
    }
    let mut no_labels = data.clone();
    no_labels.drain(label..obs);
    assert!(!opens(&no_labels));
    assert!(XptFile::open(
        data,
        XptOpenOptions {
            limits: Limits {
                max_text_bytes: 500,
                ..Default::default()
            },
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn counts_are_checked_against_extent_and_preserve_atomic_destination() {
    let good = fixture(false, Some(1));
    let obs = marker(&good, "OBSV8");
    let mut over = good.clone();
    over[obs + 48..obs + 63].copy_from_slice(b"999999999999999");
    assert!(XptFile::open(over, XptOpenOptions::default())
        .unwrap()
        .members()
        .is_err());
    let mut junk = good;
    junk.extend_from_slice(&[b'x'; 80]);
    assert!(XptFile::open(junk, XptOpenOptions::default())
        .unwrap()
        .members()
        .is_err());
    let dir = uniformat_testkit::tempdir().unwrap();
    let path = dir.path().join("count.xpt");
    std::fs::write(&path, b"previous").unwrap();
    for expected in [0, 2] {
        let mut w = XptWriter::create(
            &path,
            &schema(false),
            &options(Some(expected)),
            &Limits::default(),
        )
        .unwrap();
        let written = w.write_row(&[CellValue::Num(1.), CellValue::Text("ok")]);
        assert_eq!(written.is_err(), expected == 0);
        assert!(w.commit().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous");
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn version_specific_boundaries_and_five_digit_column_count() {
    let ws = WriteSchema {
        columns: vec![WriteColumn::character("N".repeat(32), 32767).with_label("é".repeat(128))],
    };
    assert!(Encoder::new(&ws, &options(None), &Limits::default()).is_ok());
    assert!(Encoder::new(&ws, &WriteOptions::default(), &Limits::default()).is_err());
    for bad in [
        WriteColumn::numeric("N".repeat(33), 8),
        WriteColumn::character("N", 32768),
        WriteColumn::numeric("N", 8).with_label("a".repeat(257)),
    ] {
        assert!(Encoder::new(
            &WriteSchema { columns: vec![bad] },
            &options(None),
            &Limits::default()
        )
        .is_err());
    }
    let wide = WriteSchema {
        columns: (0..10000)
            .map(|i| WriteColumn::numeric(format!("V{i}"), 8))
            .collect(),
    };
    let e = Encoder::new(&wide, &options(None), &Limits::default()).unwrap();
    assert_eq!(e.metadata().columns().len(), 10000);
    assert!(Encoder::new(&wide, &WriteOptions::default(), &Limits::default()).is_err());
}

#[test]
fn committed_fixtures_cover_long_metadata_and_counted_blanks() {
    for data in [
        include_bytes!("../../../tests/data/xpt/v8-labelv8.xpt").as_slice(),
        include_bytes!("../../../tests/data/xpt/v8-labelv9.xpt").as_slice(),
    ] {
        let f = XptFile::open(SliceSource(data), XptOpenOptions::default()).unwrap();
        assert_eq!(f.metadata().columns()[0].name, "same_prefix_number");
        assert_eq!(
            f.metadata().columns()[1].label.as_deref(),
            Some("b".repeat(256).as_str())
        );
        assert_eq!(f.members().unwrap()[0].row_count(), 2);
    }
    let data = include_bytes!("../../../tests/data/xpt/v8-counted-blank-tail.xpt");
    let f = XptFile::open(SliceSource(data), XptOpenOptions::default()).unwrap();
    assert_eq!(f.members().unwrap()[0].row_count(), 3);
}

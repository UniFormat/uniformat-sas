//! Transport reader/writer integration and atomicity guarantees.
use std::borrow::Cow;
use std::io::{self, Write};
use uniformat_sas::write::{CellValue, WriteColumn, WriteSchema};
use uniformat_sas::xpt::{Encoder, Row, TextEncoding, WriteOptions};
use uniformat_sas::{Limits, MissingKind, NumValue};
use uniformat_sas_io::{
    ByteSource, CountingSource, SliceSource, XptFile, XptOpenOptions, XptWriter,
};

fn schema() -> WriteSchema {
    WriteSchema {
        columns: vec![
            WriteColumn::numeric("VALUE", 8),
            WriteColumn::character("TEXT", 12),
        ],
    }
}
fn sample(count: usize, name: &str) -> Vec<u8> {
    let mut writer = XptWriter::new(
        Vec::new(),
        &schema(),
        &WriteOptions {
            table_name: name.into(),
            ..Default::default()
        },
        &Limits::default(),
    )
    .unwrap();
    for i in 0..count {
        writer
            .write_row(&[CellValue::Num(i as f64), CellValue::Text("été")])
            .unwrap();
    }
    writer.finish().unwrap().0
}
#[test]
fn metadata_only_open_never_reads_observations() {
    struct HeadersOnly {
        data: Vec<u8>,
        end: u64,
    }
    impl ByteSource for HeadersOnly {
        fn len(&self) -> u64 {
            self.data.len() as u64
        }
        fn read_range(&self, at: u64, len: usize) -> io::Result<Cow<'_, [u8]>> {
            assert!(at + len as u64 <= self.end, "unexpected observation read");
            self.data.read_range(at, len)
        }
    }
    let size = Encoder::new(&schema(), &WriteOptions::default(), &Limits::default())
        .unwrap()
        .header()
        .len();
    let source = CountingSource::new(HeadersOnly {
        data: sample(100, "FIRST"),
        end: size as u64,
    });
    let file = XptFile::open(source, XptOpenOptions::default()).unwrap();
    assert_eq!(file.metadata().columns().len(), 2);
    assert_eq!(file.source().bytes_read(), size as u64);
}
#[test]
fn roundtrip_empty_and_multiple_padding_shapes() {
    for count in [0, 1, 2, 3, 4, 5, 79, 80, 81, 10000] {
        let file = XptFile::open(sample(count, "DATASET"), XptOpenOptions::default()).unwrap();
        let members = file.members().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].row_count(), count as u64);
        for (i, bytes) in file.rows(&members[0]).enumerate() {
            let b = bytes.unwrap();
            let row = Row::new(&b, members[0].metadata()).unwrap();
            assert_eq!(row.number(0).unwrap(), NumValue::Value(i as f64));
            assert_eq!(row.text(1, true).unwrap(), "été");
        }
    }
}
#[test]
fn tagged_missing_values_roundtrip_through_file() {
    let ws = WriteSchema {
        columns: vec![WriteColumn::numeric("M", 8)],
    };
    let tags = vec![MissingKind::Dot, MissingKind::Underscore]
        .into_iter()
        .chain((b'A'..=b'Z').map(MissingKind::Letter))
        .collect::<Vec<_>>();
    let mut w = XptWriter::new(
        Vec::new(),
        &ws,
        &WriteOptions::default(),
        &Limits::default(),
    )
    .unwrap();
    for &tag in &tags {
        w.write_row(&[CellValue::Missing(tag)]).unwrap();
    }
    let file = XptFile::open(w.finish().unwrap().0, XptOpenOptions::default()).unwrap();
    let members = file.members().unwrap();
    assert_eq!(members[0].row_count(), 28);
    for (bytes, tag) in file.rows(&members[0]).zip(tags) {
        assert_eq!(
            Row::new(&bytes.unwrap(), members[0].metadata())
                .unwrap()
                .number(0)
                .unwrap(),
            NumValue::Missing(tag)
        );
    }
}
#[test]
fn multiple_members_include_empty_members_and_scan_boundary() {
    // 20 bytes per row; 3200 rows fill exactly one 64,000-byte indexing chunk.
    let mut a = sample(3200, "FIRST");
    let b = sample(0, "EMPTY");
    let c = sample(3, "THIRD");
    a.extend_from_slice(&b[240..]);
    a.extend_from_slice(&c[240..]);
    let file = XptFile::open(a.clone(), XptOpenOptions::default()).unwrap();
    let members = file.members().unwrap();
    assert_eq!(
        members
            .iter()
            .map(|m| (m.metadata().header().name.as_str(), m.row_count()))
            .collect::<Vec<_>>(),
        vec![("FIRST", 3200), ("EMPTY", 0), ("THIRD", 3)]
    );
    assert_eq!(file.rows(&members[2]).count(), 3);
    assert!(XptFile::open(
        a,
        XptOpenOptions {
            max_members: 2,
            ..Default::default()
        }
    )
    .unwrap()
    .members()
    .is_err());
}
#[test]
fn bad_inputs_fail_without_partial_rows() {
    let good = sample(2, "DATASET");
    for n in 0..1040 {
        assert!(XptFile::open(SliceSource(&good[..n]), XptOpenOptions::default()).is_err());
    }
    let mut bad = good.clone();
    bad.pop();
    assert!(XptFile::open(bad, XptOpenOptions::default()).is_err());
    for (at, patch) in [
        (640, vec![0, 3]),
        (644, vec![0, 9]),
        (724, vec![255; 4]),
        (960, vec![0]),
    ] {
        let mut bad = good.clone();
        bad[at..at + patch.len()].copy_from_slice(&patch);
        assert!(XptFile::open(bad, XptOpenOptions::default()).is_err());
    }
    assert!(XptFile::open(
        good,
        XptOpenOptions {
            limits: Limits {
                max_text_bytes: 100,
                ..Default::default()
            },
            ..Default::default()
        }
    )
    .is_err());
}
#[test]
fn transaction_preserves_destination_on_invalid_row_drop_and_ambiguous_tail() {
    let dir = uniformat_testkit::tempdir().unwrap();
    let dest = dir.path().join("test.xpt");
    std::fs::write(&dest, b"original").unwrap();
    {
        let mut w = XptWriter::create(
            &dest,
            &schema(),
            &WriteOptions::default(),
            &Limits::default(),
        )
        .unwrap();
        w.write_row(&[CellValue::Num(1.), CellValue::Text("ok")])
            .unwrap();
        assert!(w
            .write_row(&[CellValue::Num(f64::INFINITY), CellValue::Null])
            .is_err());
        assert!(w.write_row(&[CellValue::Num(2.), CellValue::Null]).is_err());
        assert!(w.commit().is_err());
    }
    {
        let _w = XptWriter::create(
            &dest,
            &schema(),
            &WriteOptions::default(),
            &Limits::default(),
        )
        .unwrap();
    }
    let ws = WriteSchema {
        columns: vec![WriteColumn::character("TEXT", 1)],
    };
    let mut w =
        XptWriter::create(&dest, &ws, &WriteOptions::default(), &Limits::default()).unwrap();
    w.write_row(&[CellValue::Text("x")]).unwrap();
    w.write_row(&[CellValue::Null]).unwrap();
    assert!(w.commit().is_err());
    assert_eq!(std::fs::read(&dest).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let mut w = XptWriter::create(
        &dest,
        &schema(),
        &WriteOptions::default(),
        &Limits::default(),
    )
    .unwrap();
    w.write_row(&[CellValue::Num(1.), CellValue::Text("ok")])
        .unwrap();
    assert_eq!(w.commit().unwrap(), 1);
    assert_ne!(std::fs::read(&dest).unwrap(), b"original");
}
#[test]
fn invalid_schema_does_not_touch_sink_or_create_directory_entries() {
    struct Untouchable;
    impl Write for Untouchable {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            panic!("wrote before validation")
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("flushed before validation")
        }
    }
    assert!(XptWriter::new(
        Untouchable,
        &WriteSchema::default(),
        &WriteOptions::default(),
        &Limits::default()
    )
    .is_err());
    let dir = uniformat_testkit::tempdir().unwrap();
    assert!(XptWriter::create(
        dir.path().join("bad.xpt"),
        &WriteSchema::default(),
        &WriteOptions::default(),
        &Limits::default()
    )
    .is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[test]
fn io_failure_poisoning_is_permanent() {
    struct Broken {
        remaining: usize,
    }
    impl Write for Broken {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::other("injected failure"));
            }
            let n = b.len().min(self.remaining);
            self.remaining -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut w = XptWriter::new(
        Broken { remaining: 1045 },
        &schema(),
        &WriteOptions::default(),
        &Limits::default(),
    )
    .unwrap();
    assert!(w.write_row(&[CellValue::Num(1.), CellValue::Null]).is_err());
    assert!(w.finish().is_err());
}
#[test]
fn text_encoding_is_explicit_and_lossless() {
    let ws = WriteSchema {
        columns: vec![WriteColumn::character("TEXT", 80)],
    };
    for enc in [
        TextEncoding::Utf8,
        TextEncoding::Latin1,
        TextEncoding::Windows1252,
    ] {
        let mut w = XptWriter::new(
            Vec::new(),
            &ws,
            &WriteOptions {
                encoding: enc,
                ..Default::default()
            },
            &Limits::default(),
        )
        .unwrap();
        w.write_row(&[CellValue::Text("été")]).unwrap();
        w.write_row(&[CellValue::Null]).unwrap();
        let file = XptFile::open(
            w.finish().unwrap().0,
            XptOpenOptions {
                encoding: enc,
                ..Default::default()
            },
        )
        .unwrap();
        let m = file.members().unwrap();
        assert_eq!(m[0].row_count(), 2);
        let rows = file.rows(&m[0]).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            Row::new(&rows[0], m[0].metadata())
                .unwrap()
                .text(0, true)
                .unwrap(),
            "été"
        );
        assert_eq!(
            Row::new(&rows[1], m[0].metadata())
                .unwrap()
                .text(0, true)
                .unwrap(),
            ""
        );
    }
}

#[test]
#[ignore = "generate with tests/data/xpt/interop.py or set UNIFORMAT_XPT_REFERENCE to an external fixture"]
fn independent_reference_v5() {
    let path =
        std::env::var("UNIFORMAT_XPT_REFERENCE").expect("path to a VALUE/TEXT reference fixture");
    let file = XptFile::open(
        uniformat_sas_io::FileSource::open(path).unwrap(),
        XptOpenOptions::default(),
    )
    .unwrap();
    let m = file.members().unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].row_count(), 5);
    assert_eq!(
        m[0].metadata()
            .columns()
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["VALUE", "TEXT"]
    );
    for (bytes, (num, text)) in file.rows(&m[0]).zip([
        (Some(1.), "one"),
        (Some(-1.), "moins"),
        (Some(0.5), "été"),
        (Some(100.), "cent"),
        (None, "missing"),
    ]) {
        let b = bytes.unwrap();
        let row = Row::new(&b, m[0].metadata()).unwrap();
        assert_eq!(
            row.number(0).unwrap(),
            num.map(NumValue::Value)
                .unwrap_or(NumValue::Missing(MissingKind::Dot))
        );
        assert_eq!(row.text(1, true).unwrap(), text);
    }
}

#[test]
fn writer_rejects_unescaped_member_markers_across_rows() {
    let source = sample(0, "DATASET");
    let ws = WriteSchema {
        columns: vec![WriteColumn::character("TEXT", 80)],
    };
    let mut w = XptWriter::new(
        Vec::new(),
        &ws,
        &WriteOptions::default(),
        &Limits::default(),
    )
    .unwrap();
    w.write_row(&[CellValue::Bytes(&source[240..320])]).unwrap();
    assert!(w.write_row(&[CellValue::Bytes(&source[320..400])]).is_err());
    assert!(w.finish().is_err());
}

#[test]
fn committed_fixtures_keep_the_final_numeric_missing_row() {
    for bytes in [
        include_bytes!("../../../tests/data/xpt/v5-synthetic.xpt").as_slice(),
        include_bytes!("../../../tests/data/xpt/v5-blank-tail.xpt").as_slice(),
    ] {
        let file = XptFile::open(SliceSource(bytes), XptOpenOptions::default()).unwrap();
        let m = file.members().unwrap();
        assert_eq!(m[0].row_count(), 5);
        let last = file.rows(&m[0]).last().unwrap().unwrap();
        let row = Row::new(&last, m[0].metadata()).unwrap();
        assert_eq!(row.number(0).unwrap(), NumValue::Missing(MissingKind::Dot));
        assert_eq!(
            m[0].metadata().columns()[0].label.as_deref(),
            Some("Number")
        );
    }
}

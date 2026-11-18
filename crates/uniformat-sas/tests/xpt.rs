//! Independent vectors, bounds and exact IEEE/IBM round trips for XPORT.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
use proptest::prelude::*;
use uniformat_sas::write::{CellValue, WriteColumn, WriteSchema};
use uniformat_sas::xpt::*;
use uniformat_sas::{Limits, LogicalType, MissingKind, NumValue};

#[test]
fn published_ibm_vectors_and_truncated_widths() {
    for (v, word) in [
        (0.0, 0),
        (1.0, 0x4110000000000000),
        (-1.0, 0xc110000000000000),
        (0.5, 0x4080000000000000),
        (100.0, 0x4264000000000000),
        (16.0, 0x4210000000000000),
    ] {
        assert_eq!(
            encode_number(NumValue::Value(v)).unwrap(),
            u64::to_be_bytes(word)
        );
        for width in 2..=8 {
            assert_eq!(
                decode_number(&word.to_be_bytes()[..width]).unwrap(),
                NumValue::Value(v)
            );
        }
    }
    for width in [0, 1, 9] {
        assert!(decode_number(&vec![0; width]).is_err());
    }
    assert_eq!(encode_number(NumValue::Value(-0.0)).unwrap(), [0; 8]);
}

#[test]
fn all_missing_tags_and_numeric_lookalikes() {
    for (tag, missing) in [(b'.', MissingKind::Dot), (b'_', MissingKind::Underscore)]
        .into_iter()
        .chain((b'A'..=b'Z').map(|c| (c, MissingKind::Letter(c))))
    {
        let mut expected = [0u8; 8];
        expected[0] = tag;
        assert_eq!(encode_number(NumValue::Missing(missing)).unwrap(), expected);
        for width in 2..=8 {
            assert_eq!(
                decode_number(&expected[..width]).unwrap(),
                NumValue::Missing(missing)
            );
        }
        expected[1] = 16;
        assert!(matches!(
            decode_number(&expected).unwrap(),
            NumValue::Value(_)
        ));
    }
    assert!(encode_number(NumValue::Missing(MissingKind::Letter(b'a'))).is_err());
    assert_eq!(
        encode_number(NumValue::Value(f64::NAN)).unwrap(),
        [46, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn boundaries_and_ties_round_to_even() {
    let min = 2f64.powi(-260);
    let max = f64::from_bits(2f64.powi(252).to_bits() - 1);
    for v in [min, -min, max, -max] {
        assert_eq!(
            decode_number(&encode_number(NumValue::Value(v)).unwrap()).unwrap(),
            NumValue::Value(v)
        );
    }
    for v in [
        f64::from_bits(min.to_bits() - 1),
        2f64.powi(252),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN_POSITIVE,
    ] {
        assert!(encode_number(NumValue::Value(v)).is_err());
    }
    let word = 0x4080000000000000u64;
    assert_eq!(
        decode_number(&(word + 4).to_be_bytes()).unwrap(),
        NumValue::Value(0.5)
    );
    assert_eq!(
        decode_number(&(word + 12).to_be_bytes()).unwrap(),
        NumValue::Value(f64::from_bits(0.5f64.to_bits() + 2))
    );
}
fn schema() -> WriteSchema {
    WriteSchema {
        columns: vec![
            WriteColumn::numeric("DATE", 8).with_format("DATE", 9, 0),
            WriteColumn::character("Text", 10).with_label("étiquette"),
        ],
    }
}
fn encoder() -> Encoder {
    Encoder::new(&schema(), &WriteOptions::default(), &Limits::default()).unwrap()
}

#[test]
fn metadata_rows_and_encoding_are_checked() {
    let e = encoder();
    assert_eq!(
        &e.header()[240..320],
        b"HEADER RECORD*******MEMBER  HEADER RECORD!!!!!!!000000000000000001600000000140  "
    );
    assert_eq!(e.header().len(), 1040); // 640 + 280 NAMESTR bytes + 40 padding + 80 OBS
    assert_eq!(parse_library(e.header()).unwrap(), Version::V5);
    assert_eq!(e.metadata().columns()[0].logical, LogicalType::Date);
    assert_eq!(e.metadata().columns()[1].name, "Text");
    let b = e
        .encode_row(&[CellValue::Num(0.0), CellValue::Text("été")])
        .unwrap();
    let r = Row::new(&b, e.metadata()).unwrap();
    assert_eq!(r.number(0).unwrap(), NumValue::Value(0.0));
    assert_eq!(r.text(1, true).unwrap(), "été");
    assert_eq!(r.bytes(1).unwrap().len(), 10);
    assert!(r.number(1).is_err());
    assert!(r.text(0, true).is_err());
    assert!(r.bytes(2).is_err());
    assert!(Row::new(&b[..17], e.metadata()).is_err());
    assert!(e.encode_row(&[CellValue::Num(0.0)]).is_err());
    assert!(e
        .encode_row(&[CellValue::Text("bad"), CellValue::Null])
        .is_err());
    assert!(e
        .encode_row(&[CellValue::Null, CellValue::Text("éééééé")])
        .is_err());
    assert!(TextEncoding::Utf8.decode(&[0xff]).is_err());
    assert_eq!(
        TextEncoding::Latin1.decode(&[0x80, 0xe9]).unwrap(),
        "\u{80}é"
    );
    assert_eq!(
        TextEncoding::Windows1252.decode(&[0x80, 0xe9]).unwrap(),
        "€é"
    );
}

#[test]
fn rejects_invalid_schemas_and_narrowing() {
    for cols in [
        vec![],
        vec![WriteColumn::numeric("TOO_LONG_NAME", 8)],
        vec![WriteColumn::character("a", 201)],
        vec![WriteColumn::numeric("a", 1)],
        vec![WriteColumn::numeric("A", 8), WriteColumn::numeric("a", 8)],
        vec![WriteColumn::numeric("A", 8).with_label("x".repeat(41))],
    ] {
        assert!(Encoder::new(
            &WriteSchema { columns: cols },
            &WriteOptions::default(),
            &Limits::default()
        )
        .is_err());
    }
    let narrow = Encoder::new(
        &WriteSchema {
            columns: vec![WriteColumn::numeric("A", 2)],
        },
        &WriteOptions::default(),
        &Limits::default(),
    )
    .unwrap();
    assert!(narrow.encode_row(&[CellValue::Num(1.0)]).is_ok());
    assert!(narrow.encode_row(&[CellValue::Num(1.01)]).is_err());
    for date in [
        "29FEB01:00:00:00",
        "01JAN00:24:00:00",
        "01JAN00:00:60:00",
        "00JAN00:00:00:00",
        "01JAN00",
    ] {
        assert!(Encoder::new(
            &schema(),
            &WriteOptions {
                created: date.into(),
                ..Default::default()
            },
            &Limits::default()
        )
        .is_err());
    }
    assert!(Encoder::new(
        &schema(),
        &WriteOptions {
            created: "29FEB00:00:00:00".into(),
            ..Default::default()
        },
        &Limits::default()
    )
    .is_ok());
}

#[test]
fn padding_policy_does_not_guess_or_discard_nonblank_bytes() {
    assert_eq!(observation_count(80, 8, &[b' '; 79]).unwrap(), 1); // 72 padding, not 80
    assert_eq!(observation_count(80, 1, &[b' '; 79]).unwrap(), 1);
    assert_eq!(observation_count(0, 8, &[]).unwrap(), 0);
    assert_eq!(observation_count(80, 80, &[b' '; 79]).unwrap(), 1);
    assert!(observation_count(80, 7, &[0; 79]).is_err());
    assert!(observation_count(81, 8, &[b' '; 79]).is_err());
    assert!(observation_count(80, 0, &[b' '; 79]).is_err());
}

#[test]
fn descriptors_reject_hostile_fields_and_accept_135_byte_variant() {
    let e = encoder();
    let good = &e.header()[640..960];
    for (offset, patch) in [
        (0, vec![0, 3]),
        (4, vec![0, 9]),
        (6, vec![0, 0]),
        (84, vec![255; 4]),
        (140 + 84, vec![0; 4]),
        (280, vec![0]),
    ] {
        let mut bad = good.to_vec();
        bad[offset..offset + patch.len()].copy_from_slice(&patch);
        assert!(parse_namestrs(&bad, 2, 140, TextEncoding::Utf8, &Limits::default()).is_err());
    }
    let mut short = good[..135].to_vec();
    short.extend_from_slice(&good[140..275]);
    short.resize(320, b' ');
    assert_eq!(
        parse_namestrs(&short, 2, 135, TextEncoding::Utf8, &Limits::default())
            .unwrap()
            .1,
        18
    );
    assert!(parse_namestrs(good, 10000, 140, TextEncoding::Utf8, &Limits::default()).is_err());
    assert!(parse_namestrs(
        good,
        2,
        140,
        TextEncoding::Utf8,
        &Limits {
            max_columns: 1,
            ..Default::default()
        }
    )
    .is_err());
    assert!(parse_namestrs(
        good,
        2,
        140,
        TextEncoding::Utf8,
        &Limits {
            max_row_length: 17,
            ..Default::default()
        }
    )
    .is_err());
    let mut v8 = e.header().to_vec();
    v8[20..28].copy_from_slice(b"LIBV8   ");
    assert_eq!(detect_version(&v8).unwrap(), Version::V8);
    assert!(matches!(
        parse_library(&v8),
        Err(uniformat_sas::Error::UnsupportedFormat { .. })
    ));
    for n in 0..640 {
        assert!(parse_library(&e.header()[..n]).is_err() || n >= 240);
    }
}

proptest! {
    #[test]
    fn exact_roundtrip_representable_f64(exp in -260i32..252, frac in 0u64..(1u64<<52), sign in any::<bool>()) {
        let v=f64::from_bits(((exp+1023) as u64)<<52 | frac | (u64::from(sign)<<63));
        prop_assert_eq!(decode_number(&encode_number(NumValue::Value(v)).unwrap()).unwrap(),NumValue::Value(v));
    }
    #[test]
    fn parsers_never_panic(data in prop::collection::vec(any::<u8>(),0..2048)) {
        let _=parse_library(&data);
        let _=parse_member(&data,TextEncoding::Utf8,&Limits::default());
        let _=parse_namestrs(&data,2,140,TextEncoding::Utf8,&Limits::default());
        if data.len()<=9 { let _=decode_number(&data); }
    }
}

//! Synthetic fixtures for mandatory security tests. No external SAS installation or corpus.
#![allow(dead_code)]
use std::io::Cursor;
use uniformat_sas::write::{CellValue, WriteColumn, WriteOptions, WriteSchema};
use uniformat_sas::Compression;
use uniformat_sas_io::SasWriter;

pub(crate) fn table(rows: usize, compression: Compression) -> Vec<u8> {
    let schema = WriteSchema {
        columns: vec![
            WriteColumn::numeric("x", 8),
            WriteColumn::character("text", 32),
        ],
    };
    let options = WriteOptions {
        compression,
        page_length: 4096,
        ..WriteOptions::default()
    };
    let mut writer = SasWriter::new(Cursor::new(Vec::new()), &schema, options).unwrap();
    for row in 0..rows {
        writer
            .write_row(&[
                CellValue::Num(row as f64),
                CellValue::Text("texte synthétique"),
            ])
            .unwrap();
    }
    writer.finish().unwrap().0.into_inner()
}

pub(crate) fn numeric(rows: usize) -> Vec<u8> {
    let schema = WriteSchema {
        columns: vec![WriteColumn::numeric("x", 8)],
    };
    let mut writer =
        SasWriter::new(Cursor::new(Vec::new()), &schema, WriteOptions::default()).unwrap();
    for row in 0..rows {
        writer.write_row(&[CellValue::Num(row as f64)]).unwrap();
    }
    writer.finish().unwrap().0.into_inner()
}

pub(crate) fn set_declared_rows(bytes: &mut [u8], expected: u64, rows: u64) {
    // Our writer emits the 64-bit layout. Assert the original value so this mutation fails
    // loudly if its layout changes instead of silently targeting unrelated bytes.
    assert_eq!(bytes[32], 0x33);
    let offset = bytes
        .windows(4)
        .position(|w| w == [0xf7; 4])
        .expect("RowSize subheader")
        + 48;
    assert_eq!(
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()),
        expected
    );
    bytes[offset..offset + 8].copy_from_slice(&rows.to_le_bytes());
}

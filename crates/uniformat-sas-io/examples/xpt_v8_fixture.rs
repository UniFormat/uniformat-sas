//! Reproduce the committed v8 fixtures without SAS or external data.
use std::path::Path;
use uniformat_sas::{
    write::{CellValue, WriteColumn, WriteSchema},
    xpt::{Version, WriteOptions},
    Limits, SasFormat,
};
use uniformat_sas_io::XptWriter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .ok_or("usage: xpt_v8_fixture <existing-directory>")?;
    for extended in [false, true] {
        let mut number = WriteColumn::numeric("same_prefix_number", 8).with_label(format!(
            "{}{}",
            "a".repeat(39),
            "é".repeat(80)
        ));
        if extended {
            number.format = Some(SasFormat {
                name: "LONG_FORMAT_NAME".into(),
                width: 12,
                decimals: 2,
            });
        }
        let ws = WriteSchema {
            columns: vec![
                number,
                WriteColumn::character("same_prefix_text", 250).with_label("b".repeat(256)),
            ],
        };
        let opts = WriteOptions {
            version: Version::V8,
            table_name: "LONG_DATASET_NAME".into(),
            ..Default::default()
        };
        let path = Path::new(&dir).join(if extended {
            "v8-labelv9.xpt"
        } else {
            "v8-labelv8.xpt"
        });
        let mut w = XptWriter::create(path, &ws, &opts, &Limits::default())?;
        for (n, text) in [
            (CellValue::Num(1.25), "x".repeat(250)),
            (CellValue::Null, "été".into()),
        ] {
            w.write_row(&[n, CellValue::Text(&text)])?;
        }
        w.commit()?;
    }
    let opts = WriteOptions {
        version: Version::V8,
        row_count: Some(3),
        ..Default::default()
    };
    let ws = WriteSchema {
        columns: vec![WriteColumn::character("blank_tail_text", 1)],
    };
    let mut w = XptWriter::create(
        Path::new(&dir).join("v8-counted-blank-tail.xpt"),
        &ws,
        &opts,
        &Limits::default(),
    )?;
    for text in ["x", "", ""] {
        w.write_row(&[CellValue::Text(text)])?;
    }
    w.commit()?;
    Ok(())
}

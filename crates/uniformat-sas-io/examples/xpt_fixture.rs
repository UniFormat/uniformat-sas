//! Generate a redistributable synthetic v5 fixture, independent of any licensed SAS corpus.
use uniformat_sas::write::{CellValue, WriteColumn, WriteSchema};
use uniformat_sas::xpt::WriteOptions;
use uniformat_sas::Limits;
use uniformat_sas_io::XptWriter;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: xpt_fixture <output.xpt>")?;
    let blank_tail = std::env::args().nth(2).as_deref() == Some("--blank-tail");
    let schema = WriteSchema {
        columns: vec![
            WriteColumn::numeric("VALUE", 8).with_label("Number"),
            WriteColumn::character("TEXT", if blank_tail { 12 } else { 8 }).with_label("Text"),
        ],
    };
    let mut writer =
        XptWriter::create(path, &schema, &WriteOptions::default(), &Limits::default())?;
    for (value, text) in [
        (CellValue::Num(1.), "one"),
        (CellValue::Num(-1.), "moins"),
        (CellValue::Num(0.5), "été"),
        (CellValue::Num(100.), "cent"),
        (CellValue::Null, if blank_tail { "" } else { "missing" }),
    ] {
        writer.write_row(&[value, CellValue::Text(text)])?;
    }
    writer.commit()?;
    Ok(())
}

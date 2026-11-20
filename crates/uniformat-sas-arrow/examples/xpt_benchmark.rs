//! Reproducible local throughput probe, using generated data and bounded Arrow batches.
use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray};
use std::{io::Write, sync::Arc, time::Instant};
use uniformat_sas_arrow::xpt::{XptArrowWriter, XptBatchReader, XptWriteOptions};
use uniformat_sas_io::{FileSource, XptFile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rows: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "100000".into())
        .parse()?;
    let dir = uniformat_testkit::tempdir()?;
    let path = dir.path().join("benchmark.xpt");
    let batch = RecordBatch::try_from_iter(vec![
        (
            "VALUE",
            Arc::new(Float64Array::from(
                (0..8192).map(|i| f64::from(i) / 8.0).collect::<Vec<_>>(),
            )) as ArrayRef,
        ),
        (
            "TEXT",
            Arc::new(StringArray::from(vec!["synthetic"; 8192])) as ArrayRef,
        ),
    ])?;
    let mut opts = XptWriteOptions::default();
    opts.char_lengths.insert("TEXT".into(), 32);
    let start = Instant::now();
    let mut writer = XptArrowWriter::create(&path, batch.schema(), opts)?;
    for offset in (0..rows).step_by(8192) {
        writer.write(&batch.slice(0, (rows - offset).min(8192)))?;
    }
    let written = writer.commit()?;
    let write_time = start.elapsed().as_secs_f64();
    let bytes = std::fs::metadata(&path)?.len();
    let start = Instant::now();
    let file = XptFile::open(FileSource::open(&path)?, Default::default())?;
    let mut actual = 0usize;
    for batch in XptBatchReader::new(file, Default::default(), None, None)? {
        actual += batch?.num_rows();
    }
    if actual != rows || written != rows as u64 {
        return Err("benchmark row count mismatch".into());
    }
    let read_time = start.elapsed().as_secs_f64();
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    writeln!(
        output,
        "rows,bytes,write_seconds,read_seconds,write_mib_s,read_mib_s"
    )?;
    writeln!(
        output,
        "{rows},{bytes},{write_time:.6},{read_time:.6},{:.2},{:.2}",
        bytes as f64 / 1048576.0 / write_time,
        bytes as f64 / 1048576.0 / read_time
    )?;
    Ok(())
}

#![no_main]
use libfuzzer_sys::fuzz_target;
use uniformat_sas::xpt::{self, TextEncoding};
use uniformat_sas::Limits;
use uniformat_sas_io::{SliceSource, XptFile, XptOpenOptions};

fuzz_target!(|data: &[u8]| {
    // Bound both retained metadata and work per input; malformed source fields never
    // control unbounded allocations or iteration in the harness.
    if data.len() > 1 << 20 {
        return;
    }
    let limits = Limits {
        max_columns: 128,
        max_row_length: 32_768,
        max_text_bytes: 128 << 10,
        ..Default::default()
    };
    let _ = xpt::parse_library(data);
    let _ = xpt::parse_member(data, TextEncoding::Utf8, &limits);
    let count = data.first().copied().unwrap_or(0) as usize;
    let _ = xpt::parse_namestrs(data, count, 140, TextEncoding::Utf8, &limits);
    if data.len() <= 9 {
        let _ = xpt::decode_number(data);
    }
    let opts = XptOpenOptions {
        limits,
        max_members: 8,
        ..Default::default()
    };
    if let Ok(file) = XptFile::open(SliceSource(data), opts) {
        if let Ok(members) = file.members() {
            for member in &members {
                for bytes in file.rows(member).take(128).flatten() {
                    if let Ok(row) = xpt::Row::new(&bytes, member.metadata()) {
                        for i in 0..member.metadata().columns().len() {
                            let _ = row.number(i);
                            let _ = row.text(i, true);
                        }
                    }
                }
            }
        }
    }
});

#![no_main]
//! Cible `fuzz_header` (architecture §6.4) : l'analyse d'en-tête ne panique jamais.
use libfuzzer_sys::fuzz_target;
use uniformat_sas::{HeaderParser, Limits, ReadReport, Strictness};

fuzz_target!(|data: &[u8]| {
    let mut report = ReadReport::new(Strictness::Lenient);
    let _ = HeaderParser::parse(data, &Limits::default(), &mut report);
});

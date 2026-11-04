#![no_main]
//! Cible `fuzz_rdc` (architecture §6.4) : le décompresseur RDC ne panique jamais et n'écrit jamais au-delà de `out`.
use libfuzzer_sys::fuzz_target;
use uniformat_sas::compress::rdc_decompress;

fuzz_target!(|data: &[u8]| {
    let (len, stream) = match data.split_first() {
        Some((&n, rest)) => (usize::from(n) * 16, rest),
        None => return,
    };
    let mut out = vec![0u8; len];
    let _ = rdc_decompress(stream, &mut out);
});

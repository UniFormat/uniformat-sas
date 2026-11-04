#![no_main]
//! Cible `fuzz_rle` (architecture §6.4) : le décompresseur RLE ne panique jamais et n'écrit jamais au-delà de `out`.
use libfuzzer_sys::fuzz_target;
use uniformat_sas::compress::rle_decompress;

fuzz_target!(|data: &[u8]| {
    // premier octet = longueur de sortie visée (0 à 4 095 par pas de 16), le reste = flot
    let (len, stream) = match data.split_first() {
        Some((&n, rest)) => (usize::from(n) * 16, rest),
        None => return,
    };
    let mut out = vec![0u8; len];
    let _ = rle_decompress(stream, &mut out);
});

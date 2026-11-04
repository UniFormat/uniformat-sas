#![no_main]
//! Cible `fuzz_pages` (architecture §6.4) : ouverture (en-tête, sous-en-têtes) puis lecture de toutes les lignes
//! d'un fichier arbitraire ; aucune panique, allocations bornées par `Limits`.
use libfuzzer_sys::fuzz_target;
use uniformat_sas_io::{OpenOptions, SasFile};

fuzz_target!(|data: &[u8]| {
    if let Ok(file) = SasFile::open(data.to_vec(), OpenOptions::default()) {
        let mut rows = 0u64;
        let _ = file.for_each_row(|_| rows += 1);
    }
});

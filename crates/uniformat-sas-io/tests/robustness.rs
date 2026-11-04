//! Critère de fin de J1 (3), version déterministe dans `cargo test` : ni panique ni blocage sur des entrées
//! corrompues. Le fuzzing `cargo-fuzz` (`fuzz/`) en est la version longue (nightly requis). Les octets sont
//! mutés dans l'en-tête et les premières pages de fichiers du corpus (sauté sans `UNIFORMAT_CORPUS`).

use uniformat_sas_io::{OpenOptions, SasFile};
use uniformat_testkit::Corpus;
#[path = "../../../tests/support/fixtures.rs"]
mod fixtures;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n.max(1) as u64)).unwrap_or(0)
    }
}

/// Ouvre et parcourt ; le résultat (`Ok` ou `Err`) importe peu, seule l'absence de panique compte.
fn exercise(bytes: Vec<u8>) {
    if let Ok(file) = SasFile::open(bytes, OpenOptions::default()) {
        let mut rows = 0u64;
        let _ = file.for_each_row(|_| rows += 1);
    }
}

#[test]
fn random_garbage_is_rejected_without_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for len in [0usize, 1, 31, 32, 511, 512, 513, 4096, 70_000] {
        let mut v = vec![0u8; len];
        for b in &mut v {
            *b = rng.next().to_le_bytes()[0];
        }
        assert!(SasFile::open(v, OpenOptions::default()).is_err());
    }
}

#[test]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
fn mutated_corpus_files_never_panic() {
    let corpus = Corpus::require_from_env();
    let files = corpus.files().expect("lecture du corpus");
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    let (mut cases, mut opened) = (0u32, 0u32);
    for f in files.iter().step_by(5) {
        let original = std::fs::read(&f.path).expect("lecture");
        let zone = original.len().min(1024 + 3 * 65_536);
        // troncatures à des longueurs aléatoires
        for _ in 0..8 {
            let cut = rng.below(original.len());
            exercise(original.get(..cut).unwrap_or_default().to_vec());
            cases += 1;
        }
        // mutations d'octets dans l'en-tête et les premières pages
        for _ in 0..60 {
            let mut v = original.clone();
            for _ in 0..=rng.below(4) {
                let at = rng.below(zone);
                if let Some(b) = v.get_mut(at) {
                    *b = rng.next().to_le_bytes()[0];
                }
            }
            if SasFile::open(v.clone(), OpenOptions::default()).is_ok() {
                opened += 1;
            }
            exercise(v);
            cases += 1;
        }
    }
    eprintln!("{cases} entrées corrompues, dont {opened} encore ouvrables : aucune panique");
}

#[test]
fn mutated_synthetic_files_never_panic() {
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    let mut opened = 0;
    let mut cases = 0;
    for compression in [
        uniformat_sas::Compression::None,
        uniformat_sas::Compression::Char,
        uniformat_sas::Compression::Binary,
    ] {
        let original = fixtures::table(100, compression);
        for _ in 0..8 {
            let cut = rng.below(original.len());
            exercise(original[..cut].to_vec());
            cases += 1;
        }
        for _ in 0..60 {
            let mut bytes = original.clone();
            for _ in 0..=rng.below(4) {
                let at = rng.below(bytes.len());
                bytes[at] = rng.next().to_le_bytes()[0];
            }
            opened += usize::from(SasFile::open(bytes.clone(), OpenOptions::default()).is_ok());
            exercise(bytes);
            cases += 1;
        }
    }
    assert_eq!(cases, 204);
    assert!(
        opened > 0,
        "mutations must exercise code beyond the magic signature"
    );
    eprintln!("{cases} synthetic mutations, {opened} still openable");
}

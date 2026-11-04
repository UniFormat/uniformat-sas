//! Critère de fin de J2 (2) : les vecteurs de test de `tests/data/compression_vectors/` (`15` §6) —
//! 338 vecteurs RLE et 367 vecteurs RDC extraits des fichiers compressés par SAS.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use uniformat_sas::compress::{rdc_compress, rdc_decompress, rle_compress, rle_decompress};

struct Vector {
    id: String,
    compressed: bool,
    row_length: usize,
    raw: Vec<u8>,
    stored: Vec<u8>,
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn load(name: &str) -> Option<Vec<Vector>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data/compression_vectors")
        .join(name);
    let text = std::fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .skip(1)
            .map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                Vector {
                    id: c[0].to_owned(),
                    compressed: c[4] == "compressed",
                    row_length: c[6].parse().unwrap(),
                    raw: hex(c.get(7).copied().unwrap_or("")),
                    stored: hex(c.get(8).copied().unwrap_or("")),
                }
            })
            .collect(),
    )
}

type Decompress = fn(&[u8], &mut [u8]) -> uniformat_sas::Result<()>;
type Compress = fn(&[u8], &mut Vec<u8>);

fn check(name: &str, expected_count: usize, decompress: Decompress, compress: Compress) {
    let Some(vectors) = load(name) else {
        eprintln!("{name} absent : test sauté");
        return;
    };
    assert_eq!(vectors.len(), expected_count, "{name} : nombre de vecteurs");
    let (mut known, mut unknown, mut raw_rows, mut round) = (0, 0, 0, 0);
    for v in &vectors {
        if !v.compressed {
            // ligne stockée brute : le contenu est la ligne
            if !v.raw.is_empty() {
                assert_eq!(v.raw, v.stored, "{} : ligne brute", v.id);
            }
            raw_rows += 1;
            continue;
        }
        let mut out = vec![0u8; v.row_length];
        decompress(&v.stored, &mut out).unwrap_or_else(|e| panic!("{} : {e}", v.id));
        if v.raw.is_empty() {
            unknown += 1;
        } else {
            assert_eq!(
                out, v.raw,
                "{} : décompression différente de la ligne brute",
                v.id
            );
            known += 1;
        }
        // aller-retour du compresseur sur la ligne décompressée
        let mut again = Vec::new();
        compress(&out, &mut again);
        let mut back = vec![0u8; v.row_length];
        decompress(&again, &mut back).unwrap_or_else(|e| panic!("{} : aller-retour {e}", v.id));
        assert_eq!(back, out, "{} : aller-retour", v.id);
        round += 1;
    }
    eprintln!("{name} : {known} avec ligne brute, {unknown} sans, {raw_rows} brutes, {round} allers-retours");
}

#[test]
fn rle_vectors() {
    check("char.tsv", 338, rle_decompress, rle_compress);
}

#[test]
fn rdc_vectors() {
    check("binary.tsv", 367, rdc_decompress, rdc_compress);
}

/// Le compresseur RLE reproduit les flots de SAS 9.4M7 (`15` §3.1) : vérifié sur les vecteurs du corpus généré.
#[test]
fn rle_compressor_matches_sas_streams_on_generated_corpus() {
    let Some(vectors) = load("char.tsv") else {
        return;
    };
    let (mut same, mut total) = (0, 0);
    for v in vectors.iter().filter(|v| v.compressed && !v.raw.is_empty()) {
        total += 1;
        let mut ours = Vec::new();
        rle_compress(&v.raw, &mut ours);
        same += usize::from(ours == v.stored);
    }
    eprintln!("RLE : {same} flots identiques à SAS sur {total}");
    assert!(total > 0);
    assert_eq!(same, total, "flots RLE différents de ceux de SAS");
}

//! Critère de fin de J2 (3) : `decompress(compress(x)) == x` pour des lignes aléatoires et structurées, RLE et RDC.
//! `cargo test` : proptest (cas réglables par `PROPTEST_CASES`) ; `-- --ignored` : 10⁶ lignes par algorithme.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use proptest::prelude::*;
use uniformat_sas::compress::{rdc_compress, rdc_decompress, rle_compress, rle_decompress};

fn roundtrip(row: &[u8]) {
    let mut s = Vec::new();
    rle_compress(row, &mut s);
    let mut out = vec![0u8; row.len()];
    rle_decompress(&s, &mut out).unwrap_or_else(|e| panic!("RLE {e} pour {} octets", row.len()));
    assert_eq!(out, row, "RLE");
    let mut s = Vec::new();
    rdc_compress(row, &mut s);
    let mut out = vec![0u8; row.len()];
    rdc_decompress(&s, &mut out).unwrap_or_else(|e| panic!("RDC {e} pour {} octets", row.len()));
    assert_eq!(out, row, "RDC");
}

/// Segments structurés : suites d'un octet (valeurs spéciales comprises), motifs de période 2 à 7, bruit.
fn structured_row() -> impl Strategy<Value = Vec<u8>> {
    let special = prop_oneof![Just(0x00u8), Just(0x20), Just(0x40), any::<u8>()];
    let segment = prop_oneof![
        (special, 1usize..=4200).prop_map(|(b, n)| vec![b; n]),
        (prop::collection::vec(any::<u8>(), 2..=7), 1usize..=300).prop_map(|(motif, reps)| motif
            .iter()
            .copied()
            .cycle()
            .take(motif.len() * reps)
            .collect::<Vec<u8>>()),
        prop::collection::vec(any::<u8>(), 1..=90),
    ];
    prop::collection::vec(segment, 1..=10).prop_map(|segs| segs.concat())
}

/// Décodeur strict suivant les règles de ReadStat (`sas7bdat_parse_subheader_rdc`) : une copie doit satisfaire
/// `longueur ≤ décalage` (pas de recouvrement de sa propre source). J6 : ReadStat refusait des flots valides sinon.
fn readstat_accepts_rdc(stream: &[u8], row_len: usize) -> Result<(), String> {
    let mut out: Vec<u8> = Vec::with_capacity(row_len);
    let mut i = 0usize;
    while i + 2 <= stream.len() {
        let prefix = (u16::from(stream[i]) << 8) | u16::from(stream[i + 1]);
        i += 2;
        for bit in 0..16 {
            if prefix & (1 << (15 - bit)) == 0 {
                let Some(&b) = stream.get(i) else { break };
                i += 1;
                out.push(b);
                continue;
            }
            if i + 2 > stream.len() {
                return Err("flot tronqué".into());
            }
            let (marker, next) = (stream[i], stream[i + 1]);
            i += 2;
            let (mut insert_len, mut copy_len, mut insert_byte, mut back) =
                (0usize, 0usize, 0u8, 0usize);
            if marker <= 0x0F {
                insert_len = 3 + usize::from(marker);
                insert_byte = next;
            } else if marker >> 4 == 1 {
                let b = *stream.get(i).ok_or("flot tronqué")?;
                i += 1;
                insert_len = 19 + usize::from(marker & 0x0F) + usize::from(next) * 16;
                insert_byte = b;
            } else if marker >> 4 == 2 {
                let b = *stream.get(i).ok_or("flot tronqué")?;
                i += 1;
                copy_len = 16 + usize::from(b);
                back = 3 + usize::from(marker & 0x0F) + usize::from(next) * 16;
            } else {
                copy_len = usize::from(marker >> 4);
                back = 3 + usize::from(marker & 0x0F) + usize::from(next) * 16;
            }
            if insert_len > 0 {
                out.extend(std::iter::repeat_n(insert_byte, insert_len));
            } else if copy_len > 0 {
                if out.len() < back || copy_len > back {
                    return Err(format!(
                        "copie {copy_len} > décalage {back} (ReadStat refuse)"
                    ));
                }
                let start = out.len() - back;
                out.extend_from_within(start..start + copy_len);
            }
        }
    }
    if out.len() == row_len {
        Ok(())
    } else {
        Err(format!("{} octets au lieu de {row_len}", out.len()))
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn rdc_streams_are_accepted_by_readstat_rules(row in structured_row()) {
        let mut s = Vec::new();
        rdc_compress(&row, &mut s);
        prop_assert!(readstat_accepts_rdc(&s, row.len()).is_ok(), "{:?}", readstat_accepts_rdc(&s, row.len()));
    }

    #[test]
    fn rdc_random_streams_are_accepted_by_readstat_rules(row in prop::collection::vec(any::<u8>(), 0..=700)) {
        let mut s = Vec::new();
        rdc_compress(&row, &mut s);
        prop_assert!(readstat_accepts_rdc(&s, row.len()).is_ok(), "{:?}", readstat_accepts_rdc(&s, row.len()));
    }

    #[test]
    fn random_rows_roundtrip(row in prop::collection::vec(any::<u8>(), 0..=700)) {
        roundtrip(&row);
    }

    #[test]
    fn structured_rows_roundtrip(row in structured_row()) {
        roundtrip(&row);
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }

    fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[0]
    }
}

/// 10⁶ lignes, moitié aléatoires moitié structurées, par algorithme (lancer en release).
#[test]
#[ignore = "long : cargo test --release -p uniformat-sas --test compress_props -- --ignored"]
fn one_million_rows_roundtrip() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    for i in 0..1_000_000u32 {
        let row: Vec<u8> = if i % 2 == 0 {
            let len = rng.below(400);
            (0..len).map(|_| rng.byte()).collect()
        } else {
            let mut row = Vec::new();
            for _ in 0..=rng.below(8) {
                match rng.below(3) {
                    0 => {
                        let b = [0x00, 0x20, 0x40, rng.byte()][rng.below(4)];
                        row.extend(std::iter::repeat_n(b, 1 + rng.below(1500)));
                    }
                    1 => {
                        let period = 2 + rng.below(6);
                        let motif: Vec<u8> = (0..period).map(|_| rng.byte()).collect();
                        let reps = 1 + rng.below(120);
                        row.extend(motif.iter().copied().cycle().take(period * reps));
                    }
                    _ => row.extend((0..1 + rng.below(80)).map(|_| rng.byte())),
                }
            }
            row
        };
        roundtrip(&row);
    }
}

/// Critère de fin de J2 (4), version déterministe : des flots arbitraires ne font jamais paniquer les décompresseurs
/// et l'écriture reste dans `out` (garanti par le type `&mut [u8]`) ; les cibles `fuzz_rle`/`fuzz_rdc` en sont la
/// version longue.
#[test]
fn arbitrary_streams_never_panic() {
    let mut rng = Rng(0xA5A5_5A5A_1357_9BDF);
    let (mut ok_rle, mut ok_rdc) = (0u32, 0u32);
    for _ in 0..300_000u32 {
        let len = rng.below(120);
        let stream: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        let mut out = vec![0u8; rng.below(600)];
        ok_rle += u32::from(rle_decompress(&stream, &mut out).is_ok());
        let mut out = vec![0u8; rng.below(600)];
        ok_rdc += u32::from(rdc_decompress(&stream, &mut out).is_ok());
    }
    eprintln!("300000 flots arbitraires : {ok_rle} RLE et {ok_rdc} RDC valides, aucune panique");
}

/// Flots valides mutés (un octet changé ou supprimé) : erreur ou succès, jamais de panique.
#[test]
fn mutated_valid_streams_never_panic() {
    let mut rng = Rng(0x0F0F_F0F0_2468_ACE1);
    for _ in 0..20_000u32 {
        let row: Vec<u8> = (0..rng.below(300))
            .map(|_| [0x20, 0x00, 0x61, rng.byte()][rng.below(4)])
            .collect();
        for (compress, decompress) in [
            (
                rle_compress as fn(&[u8], &mut Vec<u8>),
                rle_decompress as fn(&[u8], &mut [u8]) -> uniformat_sas::Result<()>,
            ),
            (rdc_compress, rdc_decompress),
        ] {
            let mut s = Vec::new();
            compress(&row, &mut s);
            if !s.is_empty() {
                let at = rng.below(s.len());
                s[at] = rng.byte();
                if rng.below(3) == 0 {
                    s.remove(rng.below(s.len()));
                }
            }
            let mut out = vec![0u8; row.len()];
            let _ = decompress(&s, &mut out);
        }
    }
}

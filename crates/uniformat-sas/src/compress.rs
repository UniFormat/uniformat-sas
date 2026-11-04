//! Compression des lignes : RLE (`COMPRESS=CHAR`, `SASYZCRL`) et RDC (`COMPRESS=BINARY`, `SASYZCR2`), `15` §3 et §4.
//!
//! Les décompresseurs sont écrits pour des fichiers non fiables : jamais d'écriture au-delà de `out`, jamais de
//! lecture au-delà de `src`, référence arrière bornée par les octets déjà produits, sortie exactement pleine.

use crate::bytes::slice;
use crate::error::{Error, Result};
use crate::meta::Compression;

fn bad(algorithm: &'static str, reason: &'static str) -> Error {
    Error::CorruptCompressed { algorithm, reason }
}

/// Détecte la compression d'après le premier bloc de texte (`10` §4.5) : le littéral est aux octets 12 à 19.
pub fn detect(first_text_block: &[u8]) -> Compression {
    match slice(first_text_block, 12, 8) {
        Some(b"SASYZCRL") => Compression::Char,
        Some(b"SASYZCR2") => Compression::Binary,
        _ => Compression::None,
    }
}

/// Lecteur séquentiel borné sur le flot compressé.
struct Input<'a> {
    buf: &'a [u8],
    pos: usize,
    alg: &'static str,
}

impl<'a> Input<'a> {
    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn byte(&mut self) -> Result<u8> {
        let b = self
            .buf
            .get(self.pos)
            .copied()
            .ok_or_else(|| bad(self.alg, "données manquantes"))?;
        self.pos = self.pos.saturating_add(1);
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = slice(self.buf, self.pos, n).ok_or_else(|| bad(self.alg, "données manquantes"))?;
        self.pos = self.pos.saturating_add(n);
        Ok(s)
    }
}

/// Écriture bornée dans la ligne décompressée.
struct Output<'a> {
    buf: &'a mut [u8],
    pos: usize,
    alg: &'static str,
}

impl Output<'_> {
    fn range(&self, n: usize) -> Result<std::ops::Range<usize>> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len());
        end.map(|e| self.pos..e)
            .ok_or_else(|| bad(self.alg, "sortie plus longue que row_length"))
    }

    fn fill(&mut self, byte: u8, n: usize) -> Result<()> {
        let r = self.range(n)?;
        self.buf
            .get_mut(r.clone())
            .ok_or_else(|| bad(self.alg, "sortie"))?
            .fill(byte);
        self.pos = r.end;
        Ok(())
    }

    fn copy(&mut self, src: &[u8]) -> Result<()> {
        let r = self.range(src.len())?;
        self.buf
            .get_mut(r.clone())
            .ok_or_else(|| bad(self.alg, "sortie"))?
            .copy_from_slice(src);
        self.pos = r.end;
        Ok(())
    }

    /// Copie arrière octet par octet (les zones peuvent se recouvrir).
    fn back_copy(&mut self, offset: usize, n: usize) -> Result<()> {
        if offset == 0 || offset > self.pos {
            return Err(bad(self.alg, "décalage supérieur aux octets produits"));
        }
        let r = self.range(n)?;
        for k in r.clone() {
            let v = k
                .checked_sub(offset)
                .and_then(|s| self.buf.get(s))
                .copied()
                .ok_or_else(|| bad(self.alg, "décalage"))?;
            *self.buf.get_mut(k).ok_or_else(|| bad(self.alg, "sortie"))? = v;
        }
        self.pos = r.end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.pos == self.buf.len() {
            Ok(())
        } else {
            Err(bad(self.alg, "sortie plus courte que row_length"))
        }
    }
}

/// Décompresse un flot RLE dans `out`, dont la longueur est `row_length` ; la sortie doit être exactement pleine.
///
/// # Errors
/// `CorruptCompressed` si des données manquent, si la sortie dépasse ou n'atteint pas `out.len()`.
pub fn rle_decompress(src: &[u8], out: &mut [u8]) -> Result<()> {
    const ALG: &str = "RLE";
    let mut input = Input {
        buf: src,
        pos: 0,
        alg: ALG,
    };
    let mut o = Output {
        buf: out,
        pos: 0,
        alg: ALG,
    };
    while !input.done() {
        let c = input.byte()?;
        let lo = usize::from(c & 0x0F);
        match c {
            // copie littérale longue : 64 à 16 447 octets (les bits 4 et 5 prolongent la longueur)
            0x00..=0x3F => {
                let n = usize::from(input.byte()?)
                    .saturating_add(64)
                    .saturating_add(usize::from(c & 0x3F).saturating_mul(256));
                o.copy(input.take(n)?)?;
            }
            0x40..=0x7F => {
                let n = lo
                    .saturating_mul(256)
                    .saturating_add(usize::from(input.byte()?));
                match c & 0x30 {
                    0x00 => {
                        let b = input.byte()?;
                        o.fill(b, n.saturating_add(18))?;
                    }
                    0x10 => o.fill(b'@', n.saturating_add(17))?,
                    0x20 => o.fill(b' ', n.saturating_add(17))?,
                    _ => o.fill(0, n.saturating_add(17))?,
                }
            }
            // copie littérale de 1 à 64 octets
            0x80..=0xBF => {
                let n = usize::from(c & 0x3F).saturating_add(1);
                o.copy(input.take(n)?)?;
            }
            0xC0..=0xCF => {
                let b = input.byte()?;
                o.fill(b, lo.saturating_add(3))?;
            }
            0xD0..=0xDF => o.fill(b'@', lo.saturating_add(2))?,
            0xE0..=0xEF => o.fill(b' ', lo.saturating_add(2))?,
            0xF0..=0xFF => o.fill(0, lo.saturating_add(2))?,
        }
    }
    o.finish()
}

/// Décompresse un flot RDC dans `out` (longueur `row_length`) ; la sortie doit être exactement pleine.
///
/// # Errors
/// `CorruptCompressed` si le flot est tronqué, si une référence arrière dépasse les octets produits, ou si la
/// sortie dépasse ou n'atteint pas `out.len()`.
pub fn rdc_decompress(src: &[u8], out: &mut [u8]) -> Result<()> {
    const ALG: &str = "RDC";
    let mut input = Input {
        buf: src,
        pos: 0,
        alg: ALG,
    };
    let mut o = Output {
        buf: out,
        pos: 0,
        alg: ALG,
    };
    while !input.done() {
        let ctrl = u16::from_be_bytes([
            input.byte()?,
            input
                .byte()
                .map_err(|_| bad(ALG, "mot de contrôle tronqué"))?,
        ]);
        for bit in (0..16u32).rev() {
            if input.done() {
                break;
            }
            if (ctrl >> bit) & 1 == 0 {
                o.fill(input.byte()?, 1)?;
                continue;
            }
            let u = input.byte()?;
            let (cmd, cnt) = (u >> 4, usize::from(u & 0x0F));
            match cmd {
                0 => {
                    let b = input.byte()?;
                    o.fill(b, cnt.saturating_add(3))?;
                }
                1 => {
                    let d0 = usize::from(input.byte()?);
                    let b = input.byte()?;
                    o.fill(
                        b,
                        cnt.saturating_add(19).saturating_add(d0.saturating_mul(16)),
                    )?;
                }
                _ => {
                    let d0 = usize::from(input.byte()?);
                    let offset = cnt.saturating_add(3).saturating_add(d0.saturating_mul(16));
                    let len = if cmd == 2 {
                        usize::from(input.byte()?).saturating_add(16)
                    } else {
                        usize::from(cmd)
                    };
                    o.back_copy(offset, len)?;
                }
            }
        }
    }
    o.finish()
}

// ---------------------------------------------------------------------------------------------- RLE (écriture)

fn rle_literal(out: &mut Vec<u8>, lit: &[u8]) {
    for chunk in lit.chunks(16_447) {
        let n = chunk.len();
        if n <= 64 {
            out.push(0x80u8.saturating_add(u8::try_from(n.saturating_sub(1)).unwrap_or(0)));
        } else {
            let v = n.saturating_sub(64);
            out.push(u8::try_from((v >> 8) & 0x3F).unwrap_or(0));
            out.push(u8::try_from(v & 0xFF).unwrap_or(0));
        }
        out.extend_from_slice(chunk);
    }
}

/// Émet une suite de `n` octets `b` (n ≥ 2 pour espace, `@`, nul ; ≥ 3 pour les autres), en la découpant si besoin.
fn rle_run(out: &mut Vec<u8>, b: u8, mut n: usize) {
    let special = match b {
        0x40 => Some((0x50u8, 0xD0u8)),
        0x20 => Some((0x60, 0xE0)),
        0x00 => Some((0x70, 0xF0)),
        _ => None,
    };
    while n > 0 {
        if let Some((long_cmd, short_cmd)) = special {
            if n <= 17 {
                out.push(short_cmd.saturating_add(u8::try_from(n.saturating_sub(2)).unwrap_or(0)));
                n = 0;
            } else {
                let mut m = n.min(1040);
                let rest = n.saturating_sub(m);
                if rest > 0 && rest < 2 {
                    m = n.saturating_sub(2);
                }
                let v = m.saturating_sub(17);
                out.push(long_cmd.saturating_add(u8::try_from(v >> 8).unwrap_or(0)));
                out.push(u8::try_from(v & 0xFF).unwrap_or(0));
                n = n.saturating_sub(m);
            }
        } else if n <= 18 {
            out.push(0xC0u8.saturating_add(u8::try_from(n.saturating_sub(3)).unwrap_or(0)));
            out.push(b);
            n = 0;
        } else {
            let mut m = n.min(1041);
            let rest = n.saturating_sub(m);
            if rest > 0 && rest < 3 {
                m = n.saturating_sub(3);
            }
            let v = m.saturating_sub(18);
            out.push(0x40u8.saturating_add(u8::try_from(v >> 8).unwrap_or(0)));
            out.push(u8::try_from(v & 0xFF).unwrap_or(0));
            out.push(b);
            n = n.saturating_sub(m);
        }
    }
}

/// Compresse une ligne en RLE et ajoute le flot à `out`. Les flots sont identiques à ceux de SAS sur les
/// 14 628 lignes `CHAR` du corpus complémentaire (`15` §3.1, règle de fin de ligne comprise).
///
/// Le choix brut / compressé reste à l'appelant : SAS ne stocke une ligne compressée que si le flot est
/// strictement plus court que la ligne (R11).
pub fn rle_compress(row: &[u8], out: &mut Vec<u8>) {
    let n = row.len();
    let (mut i, mut lit_start) = (0usize, 0usize);
    while i < n {
        let b = row.get(i).copied().unwrap_or(0);
        let mut j = i;
        while row.get(j) == Some(&b) {
            j = j.saturating_add(1);
        }
        let run = j.saturating_sub(i);
        let mut min_run: usize = if matches!(b, 0x40 | 0x20 | 0x00) {
            2
        } else {
            3
        };
        if j == n {
            // fin de ligne : SAS exige un octet de plus pour coder la suite
            min_run = min_run.saturating_add(1);
        }
        if run >= min_run {
            rle_literal(out, row.get(lit_start..i).unwrap_or_default());
            rle_run(out, b, run);
            lit_start = j;
        }
        i = j;
    }
    rle_literal(out, row.get(lit_start..n).unwrap_or_default());
}

// ---------------------------------------------------------------------------------------------- RDC (écriture)

const RDC_MAX_OFFSET: usize = 4098;
const RDC_MAX_LEN: usize = 271;
const RDC_MAX_RUN: usize = 4114;
/// Nombre maximal de candidats comparés par position.
const RDC_MAX_CANDIDATES: usize = 128;
/// Bornes de parcours des chaînes de hachage (collisions comprises).
const RDC_MAX_CHAIN: usize = 1024;
const RDC_HASH_BITS: u32 = 12;
const NONE: u32 = u32::MAX;

fn rdc_hash(a: u8, b: u8, c: u8) -> usize {
    let v = (u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c);
    usize::try_from(v.wrapping_mul(2_654_435_761) >> (32 - RDC_HASH_BITS)).unwrap_or(0)
}

/// Groupes de 16 éléments précédés d'un mot de contrôle big endian.
struct GroupWriter<'a> {
    out: &'a mut Vec<u8>,
    ctrl_at: Option<usize>,
    ctrl: u16,
    count: u32,
}

impl<'a> GroupWriter<'a> {
    fn new(out: &'a mut Vec<u8>) -> Self {
        Self {
            out,
            ctrl_at: None,
            ctrl: 0,
            count: 0,
        }
    }

    fn flush_ctrl(&mut self) {
        if let Some(at) = self.ctrl_at {
            if let Some(dst) = self.out.get_mut(at..at.saturating_add(2)) {
                dst.copy_from_slice(&self.ctrl.to_be_bytes());
            }
        }
    }

    fn element(&mut self, is_command: bool, bytes: &[u8]) {
        if self.count == 16 {
            self.flush_ctrl();
            self.ctrl_at = None;
        }
        if self.ctrl_at.is_none() {
            self.ctrl_at = Some(self.out.len());
            self.out.extend_from_slice(&[0, 0]);
            self.ctrl = 0;
            self.count = 0;
        }
        if is_command {
            self.ctrl |= 0x8000u16 >> self.count;
        }
        self.out.extend_from_slice(bytes);
        self.count = self.count.saturating_add(1);
    }

    fn finish(mut self) {
        self.flush_ctrl();
    }
}

/// Compresse une ligne en RDC et ajoute le flot à `out`. Tout flot produit est décodable ; il ne reproduit pas
/// octet pour octet celui de SAS (`15` §5, R9 à R11). Aucune copie ne recouvre sa propre source (longueur ≤ décalage) :
/// ReadStat refuse les autres. Le choix brut / compressé reste à l'appelant.
pub fn rdc_compress(row: &[u8], out: &mut Vec<u8>) {
    let n = row.len();
    let mut head = vec![NONE; 1 << RDC_HASH_BITS];
    let mut prev = vec![NONE; n];
    let mut w = GroupWriter::new(out);
    let at = |p: usize| row.get(p).copied().unwrap_or(0);
    let insert = |head: &mut [u32], prev: &mut [u32], p: usize| {
        if p.saturating_add(3) <= n {
            let h = rdc_hash(at(p), at(p.saturating_add(1)), at(p.saturating_add(2)));
            if let (Some(slot), Some(link), Ok(pos)) =
                (head.get_mut(h), prev.get_mut(p), u32::try_from(p))
            {
                *link = *slot;
                *slot = pos;
            }
        }
    };
    let mut pos = 0usize;
    while pos < n {
        let b = at(pos);
        let mut run = 1usize;
        while pos.saturating_add(run) < n && at(pos.saturating_add(run)) == b && run < RDC_MAX_RUN {
            run = run.saturating_add(1);
        }
        // meilleure référence arrière (décalage 3 à 4 098, longueur 3 à 271)
        let (mut best_len, mut best_ofs) = (0usize, 0usize);
        if pos.saturating_add(3) <= n {
            let h = rdc_hash(b, at(pos.saturating_add(1)), at(pos.saturating_add(2)));
            let mut cand = head.get(h).copied().unwrap_or(NONE);
            let (mut steps, mut tested) = (0usize, 0usize);
            while cand != NONE && steps < RDC_MAX_CHAIN && tested < RDC_MAX_CANDIDATES {
                steps = steps.saturating_add(1);
                let q = usize::try_from(cand).unwrap_or(usize::MAX);
                let ofs = pos.saturating_sub(q);
                if ofs > RDC_MAX_OFFSET {
                    break;
                }
                if ofs >= 3
                    && at(q) == b
                    && at(q.saturating_add(1)) == at(pos.saturating_add(1))
                    && at(q.saturating_add(2)) == at(pos.saturating_add(2))
                {
                    tested = tested.saturating_add(1);
                    let mut len = 0usize;
                    while pos.saturating_add(len) < n
                        && len < RDC_MAX_LEN
                        // pas de copie qui recouvre sa source : ReadStat refuse `longueur > décalage` (J6)
                        && len < ofs
                        && at(pos.saturating_add(len))
                            == at(pos.saturating_add(len).saturating_sub(ofs))
                    {
                        len = len.saturating_add(1);
                    }
                    if len > best_len {
                        best_len = len;
                        best_ofs = ofs;
                        if len == RDC_MAX_LEN {
                            break;
                        }
                    }
                }
                cand = prev.get(q).copied().unwrap_or(NONE);
            }
        }
        let step = if run >= 3 && run >= best_len {
            if run <= 18 {
                w.element(true, &[u8::try_from(run.saturating_sub(3)).unwrap_or(0), b]);
            } else {
                let v = run.saturating_sub(19);
                w.element(
                    true,
                    &[
                        0x10 | u8::try_from(v & 0x0F).unwrap_or(0),
                        u8::try_from(v >> 4).unwrap_or(0),
                        b,
                    ],
                );
            }
            run
        } else if best_len >= 3 {
            let v = best_ofs.saturating_sub(3);
            let (lo, hi) = (
                u8::try_from(v & 0x0F).unwrap_or(0),
                u8::try_from(v >> 4).unwrap_or(0),
            );
            if best_len <= 15 {
                w.element(true, &[(u8::try_from(best_len).unwrap_or(0) << 4) | lo, hi]);
            } else {
                w.element(
                    true,
                    &[
                        0x20 | lo,
                        hi,
                        u8::try_from(best_len.saturating_sub(16)).unwrap_or(0),
                    ],
                );
            }
            best_len
        } else {
            w.element(false, &[b]);
            1
        };
        let end = pos.saturating_add(step);
        for p in pos..end {
            insert(&mut head, &mut prev, p);
        }
        pos = end;
    }
    w.finish();
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn rle(raw: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        rle_compress(raw, &mut v);
        v
    }

    fn roundtrip_rle(raw: &[u8]) {
        let mut out = vec![0u8; raw.len()];
        rle_decompress(&rle(raw), &mut out).unwrap();
        assert_eq!(out, raw);
    }

    fn roundtrip_rdc(raw: &[u8]) {
        let mut v = Vec::new();
        rdc_compress(raw, &mut v);
        let mut out = vec![0u8; raw.len()];
        rdc_decompress(&v, &mut out).unwrap();
        assert_eq!(out, raw);
    }

    #[test]
    fn rle_spec_example() {
        // 15 §3 : 6 nuls, `F0 3F 61 62 63`, 197 espaces.
        let mut raw = vec![0u8; 6];
        raw.extend_from_slice(&[0xF0, 0x3F, 0x61, 0x62, 0x63]);
        raw.extend(std::iter::repeat_n(b' ', 197));
        let stream = [0xF4, 0x84, 0xF0, 0x3F, 0x61, 0x62, 0x63, 0x60, 0xB4];
        let mut out = vec![0u8; 208];
        rle_decompress(&stream, &mut out).unwrap();
        assert_eq!(out, raw);
        assert_eq!(rle(&raw), stream);
    }

    #[test]
    fn rdc_spec_example() {
        let mut raw = vec![0u8; 6];
        raw.extend_from_slice(&[0xF0, 0x3F, 0x61, 0x62, 0x63]);
        raw.extend(std::iter::repeat_n(b' ', 197));
        let stream = [
            0x82, 0x00, 0x03, 0x00, 0xF0, 0x3F, 0x61, 0x62, 0x63, 0x12, 0x0B, 0x20,
        ];
        let mut out = vec![0u8; 208];
        rdc_decompress(&stream, &mut out).unwrap();
        assert_eq!(out, raw);
    }

    #[test]
    fn rle_0x40_factor_is_256() {
        // 15 §3 : `41 1A 78` = 256 + 26 + 18 = 300 octets `x`.
        let mut out = vec![0u8; 300];
        rle_decompress(&[0x41, 0x1A, 0x78], &mut out).unwrap();
        assert!(out.iter().all(|&b| b == 0x78));
    }

    #[test]
    fn rle_rejects_bad_streams() {
        let mut out = [0u8; 4];
        assert!(rle_decompress(&[0x80], &mut out).is_err()); // littéral sans donnée
        assert!(rle_decompress(&[0xC0, 0x41, 0xC0, 0x41], &mut out).is_err()); // dépasse
        assert!(rle_decompress(&[0xC0, 0x41], &mut out).is_err()); // trop court (3 < 4)
        assert!(rle_decompress(&[], &mut out).is_err());
    }

    #[test]
    fn rdc_rejects_bad_streams() {
        let mut out = [0u8; 8];
        assert!(rdc_decompress(&[0x00], &mut out).is_err()); // mot de contrôle tronqué
        assert!(rdc_decompress(&[0x80, 0x00, 0x30, 0x05], &mut out).is_err()); // décalage > produit
        assert!(rdc_decompress(&[0x80, 0x00, 0x00, 0x41], &mut [0u8; 2]).is_err()); // dépasse
        assert!(rdc_decompress(&[0x00, 0x00, 0x41], &mut out).is_err()); // trop court
    }

    #[test]
    fn roundtrip_edge_rows() {
        for raw in [
            &b""[..],
            b"a",
            b"ab",
            b"aaa",
            b"aaaa",
            b"  ",
            b"   ",
            b"x  ",
            b"\0\0",
            b"@@",
            b"abcabcabcabcabcabc",
        ] {
            roundtrip_rle(raw);
            roundtrip_rdc(raw);
        }
        for byte in [0x00u8, 0x20, 0x40, 0x78, 0xFF] {
            for len in (1..=40).chain([
                63, 64, 65, 127, 129, 255, 257, 300, 1040, 1041, 4112, 4113, 4114, 4115, 4200, 9000,
            ]) {
                let mut raw = vec![byte; len];
                roundtrip_rle(&raw);
                roundtrip_rdc(&raw);
                raw.insert(0, 1);
                raw.push(2);
                roundtrip_rle(&raw);
                roundtrip_rdc(&raw);
            }
        }
    }

    #[test]
    fn long_literals_are_split() {
        let raw: Vec<u8> = (0..20_000u32)
            .map(|i| u8::try_from(i.wrapping_mul(2_654_435_761) >> 24).unwrap())
            .collect();
        roundtrip_rle(&raw);
        roundtrip_rdc(&raw);
    }

    #[test]
    fn detect_literals() {
        let mut block = vec![b' '; 40];
        assert_eq!(detect(&block), Compression::None);
        block[12..20].copy_from_slice(b"SASYZCRL");
        assert_eq!(detect(&block), Compression::Char);
        block[12..20].copy_from_slice(b"SASYZCR2");
        assert_eq!(detect(&block), Compression::Binary);
        assert_eq!(detect(&block[..15]), Compression::None);
    }
}

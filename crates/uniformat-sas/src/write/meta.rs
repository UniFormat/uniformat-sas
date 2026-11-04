//! Dispositions des colonnes, blocs de texte et sous-en-têtes de métadonnées (`20` §4.1 points 1 à 3, §5.4 et §5.5).

use super::types::{SchemaError, WriteColumn, WriteEncoding, WriteKind, WriteOptions, WriteSchema};
use crate::Compression;

pub(super) const MAX_TEXT_BLOCK: usize = 32_740;
/// Longueur de ligne minimale d'une table compressée (`20` §4.2 point 1).
pub(super) const MIN_COMPRESSED_ROW: usize = 16;
pub(super) const MAX_NAME_BYTES: usize = 32;
pub(super) const MAX_LABEL_BYTES: usize = 256;
pub(super) const MAX_CHAR_LENGTH: u32 = 32_767;
pub(super) const MAX_COLUMNS: usize = 4_000;
pub(super) const MIN_PAGE: u32 = 1_024;
pub(super) const MAX_PAGE: u32 = 1_048_576;

pub(super) fn add(a: usize, b: usize) -> usize {
    a.saturating_add(b)
}
pub(super) fn mul(a: usize, b: usize) -> usize {
    a.saturating_mul(b)
}
pub(super) fn sub(a: usize, b: usize) -> usize {
    a.saturating_sub(b)
}
pub(super) fn ceil8(a: usize) -> usize {
    mul(add(a, 7) / 8, 8)
}
pub(super) fn pad4(a: usize) -> usize {
    mul(add(a, 3) / 4, 4)
}

/// Tampon d'octets à écriture bornée : une écriture hors du tampon est ignorée (les tailles sont validées avant).
#[derive(Debug, Clone, Default)]
pub(super) struct Buf(pub(super) Vec<u8>);

impl Buf {
    pub(super) fn zeros(n: usize) -> Self {
        Self(vec![0; n])
    }

    pub(super) fn put(&mut self, off: usize, data: &[u8]) {
        if let Some(dst) = off
            .checked_add(data.len())
            .and_then(|end| self.0.get_mut(off..end))
        {
            dst.copy_from_slice(data);
        }
    }

    pub(super) fn u16(&mut self, off: usize, v: u16) {
        self.put(off, &v.to_le_bytes());
    }

    pub(super) fn u32(&mut self, off: usize, v: u32) {
        self.put(off, &v.to_le_bytes());
    }

    pub(super) fn u64(&mut self, off: usize, v: u64) {
        self.put(off, &v.to_le_bytes());
    }
}

pub(super) fn to_u64(v: usize) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

pub(super) fn to_u16(v: usize) -> u16 {
    u16::try_from(v).unwrap_or(u16::MAX)
}

/// Place d'une colonne dans une ligne.
#[derive(Debug, Clone, Copy)]
pub(super) struct ColLayout {
    pub(super) offset: usize,
    pub(super) length: usize,
    pub(super) numeric: bool,
}

/// Disposition des colonnes (`20` §4.1 point 1) : numériques d'abord par longueur décroissante, puis caractères.
pub(super) fn layout(columns: &[WriteColumn], compressed: bool) -> (Vec<ColLayout>, usize) {
    let mut order: Vec<usize> = (0..columns.len()).collect();
    // tri stable : l'ordre de création départage les égalités
    order.sort_by_key(|&i| {
        let c = columns.get(i);
        match c.map(|c| c.kind) {
            Some(WriteKind::Numeric { length }) => (0u8, std::cmp::Reverse(usize::from(length))),
            _ => (1u8, std::cmp::Reverse(0)),
        }
    });
    let mut cols = vec![
        ColLayout {
            offset: 0,
            length: 0,
            numeric: false
        };
        columns.len()
    ];
    let mut off = 0usize;
    let mut any_numeric = false;
    for i in order {
        let Some(c) = columns.get(i) else { continue };
        let numeric = matches!(c.kind, WriteKind::Numeric { .. });
        any_numeric |= numeric;
        if let Some(slot) = cols.get_mut(i) {
            *slot = ColLayout {
                offset: off,
                length: c.length(),
                numeric,
            };
        }
        off = add(off, c.length());
    }
    let row_len = if any_numeric && !compressed {
        ceil8(off)
    } else {
        off
    };
    (cols, row_len)
}

/// Blocs `ColumnText` (`20` §4.1 point 2) : chaque texte sur un multiple de 4 octets, référence (bloc, offset, longueur).
pub(super) struct Texts {
    pub(super) blocks: Vec<Vec<u8>>,
}

pub(super) type TextRef = (u16, u16, u16);

impl Texts {
    pub(super) fn new(enc_code: u8, literal: Option<&[u8; 8]>) -> Self {
        let mut first = vec![0u8; 36];
        first.splice(8..12, [0, 0, 0, enc_code]);
        first.splice(12..28, [b' '; 16]);
        first.splice(28..36, *b"DATASTEP");
        if let Some(lit) = literal {
            // table compressée : le littéral s'insère en 12..19 et le reste se décale de 8 octets
            first.splice(12..12, lit.iter().copied());
        }
        Self {
            blocks: vec![first],
        }
    }

    pub(super) fn add(&mut self, raw: &[u8]) -> TextRef {
        if raw.is_empty() {
            return (0, 0, 0);
        }
        let need = pad4(raw.len());
        let cur_len = self.blocks.last().map_or(0, Vec::len);
        if add(cur_len, need) > MAX_TEXT_BLOCK {
            self.blocks.push(vec![0u8; 8]);
        }
        let idx = sub(self.blocks.len(), 1);
        let Some(blk) = self.blocks.last_mut() else {
            return (0, 0, 0);
        };
        let r = (to_u16(idx), to_u16(blk.len()), to_u16(raw.len()));
        blk.extend_from_slice(raw);
        blk.resize(add(blk.len(), sub(need, raw.len())), 0);
        r
    }
}

const SIG_ROW: [u8; 8] = [0xf7, 0xf7, 0xf7, 0xf7, 0, 0, 0, 0];
const SIG_COL: [u8; 8] = [0xf6, 0xf6, 0xf6, 0xf6, 0, 0, 0, 0];
const SIG_COUNTS: [u8; 8] = [0x00, 0xfc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const SIG_TEXT: [u8; 8] = [0xfd, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const SIG_NAME: [u8; 8] = [0xff; 8];
const SIG_ATTR: [u8; 8] = [0xfc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const SIG_FL: [u8; 8] = [0xfe, 0xfb, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
/// Débuts de 4 octets pris pour des signatures de sous-en-tête (`ref_writer.SIG4`).
pub(super) const SIG4: [[u8; 4]; 9] = [
    [0xf7, 0xf7, 0xf7, 0xf7],
    [0xf6, 0xf6, 0xf6, 0xf6],
    [0x00, 0xfc, 0xff, 0xff],
    [0xfd, 0xff, 0xff, 0xff],
    [0xff, 0xff, 0xff, 0xff],
    [0xfc, 0xff, 0xff, 0xff],
    [0xfe, 0xff, 0xff, 0xff],
    [0xfe, 0xfb, 0xff, 0xff],
    [0xfb, 0xff, 0xff, 0xff],
];

const ROWSIZE_TAIL: [(usize, [u8; 8]); 6] = [
    (0x2A0, [0x00, 0x00, 0x08, 0x00, 0x04, 0x00, 0x00, 0x00]),
    (0x2A8, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00]),
    (0x2B0, [0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    (0x2B8, [0x00, 0x00, 0x0c, 0x00, 0x08, 0x00, 0x00, 0x00]),
    (0x2C0, [0x1c, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]),
    (0x2E8, [0x04, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00]),
];
const COMPRESSED_TAIL: [(usize, [u8; 8]); 6] = [
    (0x2A0, [0x00, 0x00, 0x08, 0x00, 0x04, 0x00, 0x00, 0x00]),
    (0x2A8, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00]),
    (0x2B0, [0x08, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x08, 0x00]),
    (0x2B8, [0x00, 0x00, 0x14, 0x00, 0x08, 0x00, 0x00, 0x00]),
    (0x2C0, [0x24, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]),
    (0x2E8, [0x04, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00]),
];

/// Un sous-en-tête de métadonnées : type de pointeur et octets.
#[derive(Debug, Clone)]
pub(super) struct Sub {
    pub(super) ptype: u8,
    pub(super) data: Vec<u8>,
}

/// Métadonnées d'une table : sous-en-têtes dans l'ordre d'écriture et positions utiles aux localisateurs.
#[derive(Debug, Clone)]
pub(super) struct Meta {
    pub(super) subs: Vec<Sub>,
    pub(super) first_fl: usize,
    pub(super) last_fl: usize,
    pub(super) total: usize,
}

fn text_sub(blk: &[u8]) -> Vec<u8> {
    let mut body = blk.to_vec();
    if body.len() >= 2 {
        body.splice(0..2, to_u16(blk.len()).to_le_bytes());
    }
    let mut sh = SIG_TEXT.to_vec();
    sh.extend_from_slice(&body);
    sh.extend_from_slice(&[0u8; 12]);
    sh
}

fn framed(sig: [u8; 8], entries: &[u8]) -> Vec<u8> {
    let mut sh = sig.to_vec();
    sh.extend_from_slice(&to_u16(add(8, entries.len())).to_le_bytes());
    sh.extend_from_slice(&[0u8; 6]);
    sh.extend_from_slice(entries);
    sh.extend_from_slice(&[0u8; 12]);
    sh
}

/// Encode un texte de métadonnées dans l'encodage cible.
fn enc_text<'a>(
    enc: WriteEncoding,
    s: &'a str,
    column: &str,
) -> Result<std::borrow::Cow<'a, [u8]>, SchemaError> {
    enc.encode(s).ok_or_else(|| SchemaError::UnencodableText {
        name: column.to_owned(),
    })
}

/// Construit les sous-en-têtes pour `row_len` et `n_rows` inconnu (RowSize patché à la fin).
pub(super) fn build(
    schema: &WriteSchema,
    cols: &[ColLayout],
    row_len: usize,
    compressed: Option<Compression>,
    opts: &WriteOptions,
) -> Result<Meta, SchemaError> {
    let enc = opts.encoding;
    let n = schema.columns.len();
    let literal: Option<&[u8; 8]> = match compressed {
        Some(Compression::Char) => Some(b"SASYZCRL"),
        Some(Compression::Binary) => Some(b"SASYZCR2"),
        _ => None,
    };
    let mut texts = Texts::new(enc.code(), literal);
    let mut refs: Vec<(TextRef, TextRef, TextRef, TextRef)> = Vec::with_capacity(n);
    let mut max_name = 1usize;
    for c in &schema.columns {
        let name = enc_text(enc, &c.name, &c.name)?;
        max_name = max_name.max(name.len());
        let name_ref = texts.add(&name);
        let fmt_ref = match &c.format {
            Some(f) => texts.add(&enc_text(enc, &f.name, &c.name)?),
            None => (0, 0, 0),
        };
        let inf_ref = match &c.informat {
            Some(f) => texts.add(&enc_text(enc, &f.name, &c.name)?),
            None => (0, 0, 0),
        };
        let lab_ref = if c.label.is_empty() {
            (0, 0, 0)
        } else {
            texts.add(&enc_text(enc, &c.label, &c.name)?)
        };
        refs.push((name_ref, fmt_ref, inf_ref, lab_ref));
    }
    let ptype: u8 = u8::from(compressed.is_some());
    let mut subs: Vec<Sub> = Vec::new();

    // RowSize
    let mut rs = Buf::zeros(808);
    rs.put(0, &SIG_ROW);
    let (marker, flags) = if compressed.is_some() {
        (0x131u64, 0x22_303Au64)
    } else {
        (0xF0, 0x22_3011)
    };
    rs.u64(8, marker);
    rs.u64(16, to_u64(add(n, 9)));
    rs.u64(32, flags);
    rs.u64(40, to_u64(row_len));
    rs.u64(72, to_u64(n));
    rs.u64(96, to_u64(n));
    rs.u64(104, u64::from(opts.page_length));
    rs.u64(128, u64::MAX);
    rs.u64(136, u64::MAX);
    rs.u16(0x2EE, to_u16(max_name));
    for (off, val) in if compressed.is_some() {
        COMPRESSED_TAIL
    } else {
        ROWSIZE_TAIL
    } {
        rs.put(off, &val);
    }
    rs.put(0x318, &[0, 0, 0, 1, 0, 0, 0, 0]);
    rs.put(0x31A, &[0, 1]);
    if compressed.is_some() {
        rs.u64(0x40, 2);
        rs.put(0x24A, &[1]);
    }
    subs.push(Sub { ptype, data: rs.0 });

    // ColumnSize
    let mut cs = Buf::zeros(24);
    cs.put(0, &SIG_COL);
    cs.u64(8, to_u64(n));
    subs.push(Sub { ptype, data: cs.0 });

    // positions (0-based) des sous-en-têtes suivants
    let text0 = 3usize;
    let text_count = texts.blocks.len();
    let name_idx = add(text0, text_count);
    let attr_idx = add(name_idx, 1);
    let first_fl = add(attr_idx, 1);
    let last_fl = sub(add(first_fl, n), 1);

    // SubheaderCounts
    let mut ct = Buf::zeros(600);
    ct.put(0, &SIG_COUNTS);
    ct.u64(8, to_u64(texts.blocks.first().map_or(0, Vec::len)));
    ct.u64(16, 3);
    ct.u64(24, 7);
    ct.u64(112, 0x070C);
    for (k, sig) in [0xFCu8, 0xFD, 0xFF, 0xFE, 0xFB, 0xFA, 0xF9]
        .into_iter()
        .enumerate()
    {
        let base = add(120, mul(40, k));
        let mut e = [0xFFu8; 8];
        if let Some(b) = e.first_mut() {
            *b = sig;
        }
        ct.put(base, &e);
    }
    for (k, idx) in [(0usize, attr_idx), (1, text0), (2, name_idx)] {
        let base = add(add(120, mul(40, k)), 8);
        let pos = to_u64(add(idx, 1));
        ct.u64(base, 1);
        ct.u64(add(base, 8), pos);
        ct.u64(add(base, 16), 1);
        ct.u64(add(base, 24), pos);
    }
    subs.push(Sub { ptype, data: ct.0 });

    for blk in &texts.blocks {
        subs.push(Sub {
            ptype: 1,
            data: text_sub(blk),
        });
    }

    // ColumnName
    let mut entries = Vec::with_capacity(mul(n, 8));
    for (nm, _, _, _) in &refs {
        for v in [nm.0, nm.1, nm.2, 0] {
            entries.extend_from_slice(&v.to_le_bytes());
        }
    }
    subs.push(Sub {
        ptype: 1,
        data: framed(SIG_NAME, &entries),
    });

    // ColumnAttributes
    let mut ent = Vec::with_capacity(mul(n, 16));
    for (c, l) in schema.columns.iter().zip(cols) {
        let name_len = enc.encode(&c.name).map_or(0, |b| b.len());
        let flags: u16 = if name_len <= 8 { 0x0400 } else { 0x0800 };
        ent.extend_from_slice(&to_u64(l.offset).to_le_bytes());
        ent.extend_from_slice(&u32::try_from(l.length).unwrap_or(u32::MAX).to_le_bytes());
        ent.extend_from_slice(&flags.to_le_bytes());
        ent.push(if l.numeric { 1 } else { 2 });
        ent.push(0);
    }
    subs.push(Sub {
        ptype: 1,
        data: framed(SIG_ATTR, &ent),
    });

    // FormatAndLabel
    for (c, (_, fr, ir, lr)) in schema.columns.iter().zip(&refs) {
        let mut fl = Buf::zeros(64);
        fl.put(0, &SIG_FL);
        let (fw, fd) = c.format.as_ref().map_or((0, 0), |f| (f.width, f.decimals));
        let (iw, id) = c
            .informat
            .as_ref()
            .map_or((0, 0), |f| (f.width, f.decimals));
        for (k, v) in [fw, fd, iw, id].into_iter().enumerate() {
            fl.u16(add(0x18, mul(2, k)), v);
        }
        for (base, r) in [(0x28usize, ir), (0x2E, fr), (0x34, lr)] {
            fl.u16(base, r.0);
            fl.u16(add(base, 2), r.1);
            fl.u16(add(base, 4), r.2);
        }
        subs.push(Sub { ptype, data: fl.0 });
    }
    let total = subs.iter().map(|s| s.data.len()).sum();
    Ok(Meta {
        subs,
        first_fl,
        last_fl,
        total,
    })
}

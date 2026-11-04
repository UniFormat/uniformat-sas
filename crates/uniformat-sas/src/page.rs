//! Pages, lignes et valeurs (`10` §3 et §5).

use std::borrow::Cow;

use encoding_rs::Encoding;

use crate::bytes::{slice, to_usize, Endian, Layout};
use crate::encoding::{blank_byte, encoding_for_code, trim_text};
use crate::error::{Error, Result};
use crate::header::FileHeader;
use crate::meta::{Column, ColumnKind, Compression, TableMeta};
use crate::report::{ReadReport, Strictness};

/// Nature d'une page, d'après les bits du type (`10` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    /// `0x0000` : sous-en-têtes seulement.
    Meta,
    /// `0x0100` : lignes seulement.
    Data,
    /// `0x0200` / `0x0280` : sous-en-têtes puis lignes.
    Mixed,
    /// `0x0400` : page d'index (sous-en-tête `FFFFFFFA`).
    Amd,
    /// `0x4000` : rôle inconnu.
    Meta2,
    /// `0x8000` (donc `0x9000`) : page sans ligne propre aux tables compressées.
    Compressed,
    /// Autre combinaison de bits : ignorée.
    Other,
}

/// En-tête d'une page (40 octets en 64 bits, 24 en 32 bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageHeader {
    /// Numéro de page (base 0).
    pub index: u64,
    /// Type brut.
    pub page_type: u16,
    /// Nature décodée.
    pub kind: PageKind,
    /// Sous-en-têtes (pointeur terminal compris) plus lignes.
    pub block_count: u16,
    /// Pointeurs de sous-en-têtes.
    pub subheader_count: u16,
    /// Octets libres déclarés (`free_bytes`), utilisés pour situer le bitmap de suppression.
    pub free_bytes: u64,
}

impl PageHeader {
    /// Longueur de l'en-tête de page : 40 octets en 64 bits, 24 en 32 bits.
    pub fn header_len(h: &FileHeader) -> usize {
        h.layout().po().saturating_add(8)
    }

    /// Décode l'en-tête de `page` (page entière) et contrôle la signature de page (U06).
    pub(crate) fn parse(
        h: &FileHeader,
        index: u64,
        page: &[u8],
        report: &mut ReadReport,
    ) -> Result<Self> {
        if usize::try_from(h.page_length).is_ok_and(|p| page.len() < p) {
            return Err(Error::Truncated {
                needed: u64::from(h.page_length),
                got: page.len() as u64,
            });
        }
        Self::parse_prefix(h, index, page, report)
    }

    /// Décode l'en-tête de page à partir de ses seuls [`PageHeader::header_len`] premiers octets : permet de
    /// planifier la lecture sans transférer les pages (ADR 0002, plan de pages).
    ///
    /// # Errors
    /// `CorruptPage` si `bytes` est plus court que l'en-tête ; U06 en mode strict.
    pub fn parse_prefix(
        h: &FileHeader,
        index: u64,
        bytes: &[u8],
        report: &mut ReadReport,
    ) -> Result<Self> {
        let page = bytes;
        let l = h.layout();
        let bad = |field| Error::CorruptPage { page: index, field };
        let po = l.po();
        let page_type = l.endian.u16(page, po).ok_or_else(|| bad("type de page"))?;
        let block_count = l
            .endian
            .u16(page, po.saturating_add(2))
            .ok_or_else(|| bad("block_count"))?;
        let subheader_count = l
            .endian
            .u16(page, po.saturating_add(4))
            .ok_or_else(|| bad("subheader_count"))?;
        let free_off = if l.w64 { 24 } else { 12 };
        let free_bytes = l.word(page, free_off).ok_or_else(|| bad("free_bytes"))?;
        let sig = l
            .endian
            .u32(page, 0)
            .ok_or_else(|| bad("signature de page"))?;
        if h.file_code != 0
            && sig != h.file_code ^ (u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX))
        {
            report.warn(
                "U06",
                Some(index),
                "signature de page différente de file_code ^ (k + 1)",
            )?;
        }
        let kind = match page_type & 0x0F00 {
            0x0100 => PageKind::Data,
            0x0200 => PageKind::Mixed,
            0x0400 => PageKind::Amd,
            0x0000 if page_type & 0x8000 != 0 => PageKind::Compressed,
            0x0000 if page_type & 0x4000 != 0 => PageKind::Meta2,
            0x0000 => PageKind::Meta,
            _ => PageKind::Other,
        };
        Ok(Self {
            index,
            page_type,
            kind,
            block_count,
            subheader_count,
            free_bytes,
        })
    }

    /// Lignes physiques portées par la page (suppressions comprises).
    pub fn physical_rows(&self) -> Result<u32> {
        match self.kind {
            PageKind::Data => Ok(u32::from(self.block_count)),
            PageKind::Mixed => self
                .block_count
                .checked_sub(self.subheader_count)
                .map(u32::from)
                .ok_or(Error::CorruptPage {
                    page: self.index,
                    field: "block_count < subheader_count",
                }),
            _ => Ok(0),
        }
    }

    /// Vrai si le bitmap de lignes supprimées est significatif (`0x0080`).
    pub fn has_deleted(&self) -> bool {
        self.page_type & 0x0080 != 0
    }
}

/// Code d'une valeur manquante SAS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingKind {
    /// `.`
    Dot,
    /// `._`
    Underscore,
    /// `.A` à `.Z` (octet ASCII de la lettre).
    Letter(u8),
}

/// Valeur numérique : nombre ou manquant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumValue {
    /// Valeur réelle.
    Value(f64),
    /// Manquant.
    Missing(MissingKind),
}

/// Contexte de décodage du texte d'une table.
#[derive(Debug, Clone, Copy)]
struct TextCtx {
    encoding: Option<&'static Encoding>,
    /// Octet de blanc de remplissage (`0x20`, `0x40` en EBCDIC).
    blank: u8,
    /// Code d'encodage de l'en-tête.
    code: u8,
    /// Encodage imposé par l'appelant (U22).
    forced: bool,
}

impl TextCtx {
    fn for_code(code: u8) -> Self {
        Self {
            encoding: encoding_for_code(code),
            blank: blank_byte(code),
            code,
            forced: false,
        }
    }
}

/// Que faire d'un champ caractère dont les octets sont invalides pour l'encodage (`30` §6.2) ?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InvalidBytes {
    /// Remplacer chaque séquence invalide par U+FFFD ; [`TextValue::Text::replaced`] le signale (défaut).
    #[default]
    Replace,
    /// Rendre `Error::Encoding`.
    Error,
    /// Rendre les octets bruts ([`TextValue::Bytes`]) : relecture octet pour octet, comme SAS.
    Binary,
}

/// Politique de lecture d'un champ caractère.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPolicy {
    /// Traitement des octets invalides.
    pub invalid: InvalidBytes,
    /// Retirer les blancs de fin (défaut) ; les blancs de tête sont toujours conservés.
    pub trim_trailing: bool,
}

impl Default for TextPolicy {
    fn default() -> Self {
        Self {
            invalid: InvalidBytes::Replace,
            trim_trailing: true,
        }
    }
}

/// Valeur d'un champ caractère lue avec une [`TextPolicy`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextValue<'p> {
    /// Texte décodé ; `replaced` est vrai si des octets invalides ont été remplacés.
    Text {
        /// Texte.
        value: Cow<'p, str>,
        /// Au moins une séquence invalide a été remplacée par U+FFFD.
        replaced: bool,
    },
    /// Octets bruts (mode `Binary` sur octets invalides, ou encodage sans équivalent standard).
    Bytes(&'p [u8]),
}

/// Une ligne visible d'une page.
#[derive(Debug, Clone, Copy)]
pub struct RowRef<'p> {
    row: &'p [u8],
    endian: Endian,
    encoding: TextCtx,
    index: u64,
}

impl<'p> RowRef<'p> {
    /// Indice de la ligne physique dans la table (suppressions comprises, base 0).
    pub fn physical_index(&self) -> u64 {
        self.index
    }

    /// Octets de la ligne entière.
    pub fn raw(&self) -> &'p [u8] {
        self.row
    }

    /// Octets bruts du champ de `col`, blancs finaux compris.
    pub fn bytes(&self, col: &Column) -> &'p [u8] {
        slice(
            self.row,
            to_usize(u64::from(col.offset)).unwrap_or(usize::MAX),
            col.width(),
        )
        .unwrap_or_default()
    }

    /// Octets du champ caractère sans blancs de fin ; un champ entièrement nul est vide (`10` §5.4).
    pub fn char_bytes(&self, col: &Column) -> &'p [u8] {
        let f = self.bytes(col);
        if f.iter().all(|&b| b == 0) {
            return &[];
        }
        let end = f
            .iter()
            .rposition(|&b| b != self.encoding.blank)
            .map_or(0, |p| p.saturating_add(1));
        f.get(..end).unwrap_or_default()
    }

    /// Texte décodé d'une colonne caractère (octets invalides remplacés par U+FFFD ; politique complète : J3).
    pub fn text(&self, col: &Column) -> Cow<'p, str> {
        let raw = self.char_bytes(col);
        match self.encoding.encoding {
            Some(enc) => match enc.decode_without_bom_handling(raw).0 {
                Cow::Borrowed(s) => Cow::Borrowed(trim_text(s)),
                Cow::Owned(s) => Cow::Owned(trim_text(&s).to_owned()),
            },
            None => Cow::Owned(
                trim_text(&encoding_rs::WINDOWS_1252.decode_without_bom_handling(raw).0).to_owned(),
            ),
        }
    }

    /// Texte d'une colonne caractère selon `policy` (`30` §6.2) : octets invalides remplacés, refusés, ou rendus bruts.
    /// Un encodage sans équivalent standard n'est lisible qu'en `Binary`.
    ///
    /// # Errors
    /// `Encoding` en mode `Error` sur octets invalides ; `UnknownEncoding` si l'encodage n'a pas d'équivalent
    /// standard et que le mode n'est pas `Binary`.
    pub fn text_value(&self, col: &Column, policy: &TextPolicy) -> Result<TextValue<'p>> {
        let raw = if policy.trim_trailing {
            self.char_bytes(col)
        } else {
            self.bytes(col)
        };
        let Some(enc) = self.encoding.encoding else {
            return match policy.invalid {
                InvalidBytes::Binary => Ok(TextValue::Bytes(raw)),
                _ => Err(Error::UnknownEncoding(self.encoding.code)),
            };
        };
        if let Some(value) = enc.decode_without_bom_handling_and_without_replacement(raw) {
            return Ok(TextValue::Text {
                value,
                replaced: false,
            });
        }
        match policy.invalid {
            InvalidBytes::Replace => Ok(TextValue::Text {
                value: Cow::Owned(enc.decode_without_bom_handling(raw).0.into_owned()),
                replaced: true,
            }),
            InvalidBytes::Error => Err(Error::Encoding {
                column: col.index,
                row: self.index,
            }),
            InvalidBytes::Binary => Ok(TextValue::Bytes(raw)),
        }
    }

    /// Valeur d'une colonne numérique (largeur 3 à 8 : octets de poids faible complétés par des zéros, `10` §5.2).
    pub fn num(&self, col: &Column) -> NumValue {
        let f = self.bytes(col);
        let n = f.len().min(8);
        let mut buf = [0u8; 8];
        match self.endian {
            Endian::Little => {
                if let (Some(dst), Some(src)) =
                    (buf.get_mut(8usize.saturating_sub(n)..), f.get(..n))
                {
                    dst.copy_from_slice(src);
                }
            }
            Endian::Big => {
                if let (Some(dst), Some(src)) = (buf.get_mut(..n), f.get(..n)) {
                    dst.copy_from_slice(src);
                }
            }
        }
        let v = match self.endian {
            Endian::Little => f64::from_le_bytes(buf),
            Endian::Big => f64::from_be_bytes(buf),
        };
        if !v.is_nan() {
            return NumValue::Value(v);
        }
        // Le code du manquant est l'octet de rang 5 (little endian) ou 2 (big endian) du motif.
        let tag = match self.endian {
            Endian::Little => buf.get(5),
            Endian::Big => buf.get(2),
        };
        NumValue::Missing(match tag.copied().unwrap_or(0) {
            0xFF => MissingKind::Underscore,
            t @ 0xE4..=0xFD => MissingKind::Letter(b'A'.saturating_add(0xFDu8.saturating_sub(t))),
            _ => MissingKind::Dot,
        })
    }
}

/// Lignes visibles d'une page.
#[derive(Debug)]
pub struct PageRows<'p> {
    page: &'p [u8],
    start: usize,
    row_length: usize,
    count: usize,
    bitmap: Option<&'p [u8]>,
    next: usize,
    first_index: u64,
    endian: Endian,
    encoding: TextCtx,
}

impl<'p> Iterator for PageRows<'p> {
    type Item = RowRef<'p>;

    fn next(&mut self) -> Option<RowRef<'p>> {
        while self.next < self.count {
            let i = self.next;
            self.next = i.saturating_add(1);
            if let Some(bm) = self.bitmap {
                let byte = bm.get(i / 8).copied().unwrap_or(0);
                if byte & (0x80u8 >> (i % 8)) != 0 {
                    continue;
                }
            }
            let off = self.start.saturating_add(i.saturating_mul(self.row_length));
            let row = slice(self.page, off, self.row_length)?;
            return Some(RowRef {
                row,
                endian: self.endian,
                encoding: self.encoding,
                index: self.first_index.saturating_add(i as u64),
            });
        }
        None
    }
}

/// État d'un [`PageDecoder`] détaché de la table : permet de le conserver entre deux appels sans emprunter les
/// métadonnées (adaptateurs asynchrones, flux de lots).
#[derive(Debug)]
pub struct DecoderState {
    encoding: TextCtx,
    report: ReadReport,
    rows_seen: u64,
    scratch: Vec<u8>,
    u22_checked: bool,
}

impl DecoderState {
    /// État initial : avant la première ligne, avec l'encodage de l'en-tête ou `encoding` s'il est imposé.
    pub fn new(
        meta: &TableMeta,
        strictness: Strictness,
        encoding: Option<&'static Encoding>,
    ) -> Self {
        let mut ctx = TextCtx::for_code(meta.header.encoding_code);
        if let Some(enc) = encoding {
            ctx.encoding = Some(enc);
            ctx.forced = true;
        }
        Self {
            encoding: ctx,
            report: ReadReport::new(strictness),
            rows_seen: 0,
            scratch: Vec::new(),
            u22_checked: false,
        }
    }

    /// Positionne l'état après `rows_before` lignes physiques (voir [`PageDecoder::starting_at`]).
    #[must_use]
    pub fn starting_at(mut self, rows_before: u64) -> Self {
        self.rows_seen = rows_before;
        self
    }

    /// Avertissements émis jusqu'ici.
    pub fn report(&self) -> &ReadReport {
        &self.report
    }

    /// Lignes physiques déjà parcourues.
    pub fn rows_seen(&self) -> u64 {
        self.rows_seen
    }
}

/// Décodeur de lignes pour une table décrite par `meta`.
#[derive(Debug)]
pub struct PageDecoder<'m> {
    meta: &'m TableMeta,
    layout: Layout,
    encoding: TextCtx,
    report: ReadReport,
    rows_seen: u64,
    scratch: Vec<u8>,
    u22_checked: bool,
}

impl<'m> PageDecoder<'m> {
    /// Décodeur positionné avant la première ligne.
    pub fn new(meta: &'m TableMeta, strictness: Strictness) -> Self {
        Self {
            meta,
            layout: meta.header.layout(),
            encoding: TextCtx::for_code(meta.header.encoding_code),
            report: ReadReport::new(strictness),
            rows_seen: 0,
            scratch: Vec::new(),
            u22_checked: false,
        }
    }

    /// Reprend un décodeur depuis son état (voir [`PageDecoder::into_state`]).
    pub fn resume(meta: &'m TableMeta, state: DecoderState) -> Self {
        Self {
            meta,
            layout: meta.header.layout(),
            encoding: state.encoding,
            report: state.report,
            rows_seen: state.rows_seen,
            scratch: state.scratch,
            u22_checked: state.u22_checked,
        }
    }

    /// Détache l'état du décodeur (il n'emprunte plus les métadonnées).
    pub fn into_state(self) -> DecoderState {
        DecoderState {
            encoding: self.encoding,
            report: self.report,
            rows_seen: self.rows_seen,
            scratch: self.scratch,
            u22_checked: self.u22_checked,
        }
    }

    /// Positionne le décodeur après `rows_before` lignes physiques : pour décoder une plage de pages sans lire les
    /// précédentes (table non compressée ; `rows_before` plafonne les lignes lues à `row_count`). Une table
    /// compressée se lit toujours depuis la première page.
    #[must_use]
    pub fn starting_at(mut self, rows_before: u64) -> Self {
        self.rows_seen = rows_before;
        self
    }

    /// Impose l'encodage de décodage du texte des lignes (priorité sur l'octet 70, U22).
    #[must_use]
    pub fn with_encoding(mut self, encoding: &'static Encoding) -> Self {
        self.encoding.encoding = Some(encoding);
        self.encoding.forced = true;
        self
    }

    /// Avertissements émis pendant le décodage.
    pub fn report(&self) -> &ReadReport {
        &self.report
    }

    /// Appelle `f` pour chaque ligne visible de la page suivante, table compressée ou non ; les pages doivent
    /// être fournies dans l'ordre du fichier. Une ligne compressée est décompressée dans un tampon réutilisé.
    ///
    /// # Errors
    /// `CorruptPage` si les lignes, le bitmap ou un pointeur sortent de la page ; `CorruptCompressed` si un
    /// flot RLE ou RDC est invalide.
    pub fn visit_rows<F: FnMut(RowRef<'_>)>(
        &mut self,
        page_index: u64,
        page: &[u8],
        mut f: F,
    ) -> Result<()> {
        // U22 : un fichier déclaré latin1/wlatin1 (29, 62) dont les premières lignes contiennent de l'UTF-8 valide
        // non ASCII est probablement mal étiqueté ; on le signale une fois (le contrôle porte sur 256 lignes).
        let meta = self.meta;
        let probe = !self.u22_checked
            && !self.encoding.forced
            && matches!(self.encoding.code, 29 | 62)
            && self.rows_seen < 256;
        let mut utf8_seen = false;
        let mut g = |row: RowRef<'_>| {
            if probe && !utf8_seen {
                utf8_seen = meta.columns.iter().any(|c| {
                    matches!(c.kind, ColumnKind::Char { .. }) && {
                        let b = row.char_bytes(c);
                        !b.is_ascii() && std::str::from_utf8(b).is_ok()
                    }
                });
            }
            f(row);
        };
        if self.meta.compression == Compression::None {
            for row in self.rows(page_index, page)? {
                g(row);
            }
        } else {
            self.visit_compressed(page_index, page, &mut g)?;
        }
        if utf8_seen {
            self.u22_checked = true;
            self.report.warn(
                "U22",
                Some(page_index),
                "code d'encodage 29 ou 62 mais octets UTF-8 valides : envisager encoding_override",
            )?;
        }
        Ok(())
    }

    /// Lignes d'une table compressée : chacune est un sous-en-tête de la page (`15` §2), compressé (bit 2 de
    /// l'octet de compression), ou brut (longueur `row_length` sans signature de métadonnées).
    fn visit_compressed<F: FnMut(RowRef<'_>)>(
        &mut self,
        page_index: u64,
        page: &[u8],
        f: &mut F,
    ) -> Result<()> {
        let ph = PageHeader::parse(&self.meta.header, page_index, page, &mut self.report)?;
        if !matches!(
            ph.kind,
            PageKind::Meta | PageKind::Mixed | PageKind::Amd | PageKind::Meta2
        ) {
            return Ok(());
        }
        let l = self.layout;
        let (w, ps) = (l.w(), l.ps());
        let w2 = w.saturating_mul(2);
        let row_length = usize::try_from(self.meta.row_length).unwrap_or(usize::MAX);
        let bad = |field| Error::CorruptPage {
            page: page_index,
            field,
        };
        let base = l.po().saturating_add(8);
        let alg = self.meta.compression;
        if self.scratch.len() != row_length {
            self.scratch = vec![0u8; row_length];
        }
        for i in 0..usize::from(ph.subheader_count) {
            if self.rows_seen >= self.meta.row_count {
                break;
            }
            let q = base.saturating_add(i.saturating_mul(ps));
            let (Some(off), Some(len), Some(&comp)) = (
                l.word(page, q),
                l.word(page, q.saturating_add(w)),
                page.get(q.saturating_add(w2)),
            ) else {
                return Err(bad("pointeur de sous-en-tête"));
            };
            if len == 0 || comp == 1 {
                continue;
            }
            let sh = to_usize(off)
                .zip(to_usize(len))
                .and_then(|(o, n)| slice(page, o, n))
                .ok_or_else(|| bad("sous-en-tête hors page"))?;
            let index = self.rows_seen;
            let (endian, encoding) = (l.endian, self.encoding);
            // C09 : le bit 3 de l'octet de compression marque une ligne supprimée (readstat/compression_type_0x09
            // et 0x0d : `deleted_count = 1`, octets 9 et 13) ; hypothèse sur deux fichiers.
            let deleted = comp & 0x08 != 0;
            if deleted {
                self.report.warn(
                    "C09",
                    Some(page_index),
                    "bit 3 de l'octet de compression : ligne lue comme supprimée",
                )?;
            }
            if comp & 4 != 0 {
                if sh.len() == row_length {
                    // longueur stockée = row_length : ligne brute (décodeur de SAS, 15 §3.1)
                    if !deleted {
                        f(RowRef {
                            row: sh,
                            endian,
                            encoding,
                            index,
                        });
                    }
                } else {
                    match alg {
                        Compression::Char => {
                            crate::compress::rle_decompress(sh, &mut self.scratch)?
                        }
                        _ => crate::compress::rdc_decompress(sh, &mut self.scratch)?,
                    }
                    if !deleted {
                        f(RowRef {
                            row: &self.scratch,
                            endian,
                            encoding,
                            index,
                        });
                    }
                }
            } else if sh.len() == row_length
                && sh.len() >= w2
                && crate::meta::signature(l, sh)
                    .is_none_or(|s| !crate::meta::KNOWN_SIGNATURES.contains(&s))
            {
                if !deleted {
                    f(RowRef {
                        row: sh,
                        endian,
                        encoding,
                        index,
                    });
                }
            } else {
                continue;
            }
            self.rows_seen = self.rows_seen.saturating_add(1);
        }
        Ok(())
    }

    /// Lignes visibles de la page suivante d'une table **non compressée** ; les pages doivent être fournies dans
    /// l'ordre du fichier.
    ///
    /// # Errors
    /// `Unsupported` pour une table compressée (utiliser [`PageDecoder::visit_rows`]), `CorruptPage` si les
    /// lignes ou le bitmap sortent de la page.
    pub fn rows<'p>(&mut self, page_index: u64, page: &'p [u8]) -> Result<PageRows<'p>> {
        let first = self.rows_seen;
        let (rows, physical) = self.rows_from(page_index, page, first)?;
        self.rows_seen = first.saturating_add(physical);
        Ok(rows)
    }

    /// Comme [`PageDecoder::rows`] pour une page lue hors séquence : `rows_before` est le nombre de lignes
    /// physiques des pages précédentes (plafond `row_count`). Rend aussi les lignes physiques de la page.
    ///
    /// # Errors
    /// Mêmes erreurs que [`PageDecoder::rows`].
    pub fn rows_from<'p>(
        &mut self,
        page_index: u64,
        page: &'p [u8],
        rows_before: u64,
    ) -> Result<(PageRows<'p>, u64)> {
        if self.meta.compression != Compression::None {
            return Err(Error::Unsupported(
                "lignes compressées : utiliser visit_rows",
            ));
        }
        let ph = PageHeader::parse(&self.meta.header, page_index, page, &mut self.report)?;
        let l = self.layout;
        let bad = |field| Error::CorruptPage {
            page: page_index,
            field,
        };
        let physical = ph.physical_rows()?;
        let n = usize::try_from(physical).map_err(|_| bad("lignes"))?;
        let po8 = l.po().saturating_add(8);
        let start = match ph.kind {
            PageKind::Data => po8,
            PageKind::Mixed => {
                let ptrs_end =
                    po8.saturating_add(usize::from(ph.subheader_count).saturating_mul(l.ps()));
                ptrs_end.saturating_add(7).div_euclid(8).saturating_mul(8)
            }
            _ => 0,
        };
        let row_length = usize::try_from(self.meta.row_length).map_err(|_| bad("row_length"))?;
        let rows_end = start.saturating_add(n.saturating_mul(row_length));
        if n > 0 && rows_end > page.len() {
            return Err(bad("lignes hors de la page"));
        }
        let bitmap = if ph.has_deleted() && n > 0 {
            if !matches!(ph.kind, PageKind::Mixed) || !l.w64 || l.endian != Endian::Little {
                self.report.warn(
                    "U19",
                    Some(page_index),
                    "bitmap de suppression hors du cas observé",
                )?;
            }
            let off = to_usize(ph.free_bytes)
                .and_then(|f| f.checked_add(rows_end))
                .ok_or_else(|| bad("bitmap"))?;
            Some(
                slice(page, off, n.div_ceil(8))
                    .ok_or_else(|| bad("bitmap de suppression hors de la page"))?,
            )
        } else {
            None
        };
        // Le plafond row_count s'applique après la lecture du bitmap.
        let room = self.meta.row_count.saturating_sub(rows_before);
        let count = usize::try_from(room).map_or(n, |r| r.min(n));
        let rows = PageRows {
            page,
            start,
            row_length,
            count,
            bitmap,
            next: 0,
            first_index: rows_before,
            endian: l.endian,
            encoding: self.encoding,
        };
        Ok((rows, u64::from(physical)))
    }
}

impl ColumnKind {
    /// Vrai pour une colonne numérique.
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Numeric { .. })
    }
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

    fn num_col(length: u8) -> Column {
        Column {
            index: 0,
            name: "x".into(),
            label: None,
            kind: ColumnKind::Numeric { length },
            offset: 0,
            format: None,
            informat: None,
            indexed: false,
            logical: crate::formats::LogicalType::Number,
        }
    }

    fn row(bytes: &[u8], endian: Endian) -> RowRef<'_> {
        RowRef {
            row: bytes,
            endian,
            encoding: TextCtx {
                encoding: None,
                blank: b' ',
                code: 0,
                forced: false,
            },
            index: 0,
        }
    }

    #[test]
    fn full_width_double_little_and_big_endian() {
        let v = 1234.5678_f64;
        assert_eq!(
            row(&v.to_le_bytes(), Endian::Little).num(&num_col(8)),
            NumValue::Value(v)
        );
        assert_eq!(
            row(&v.to_be_bytes(), Endian::Big).num(&num_col(8)),
            NumValue::Value(v)
        );
    }

    #[test]
    fn truncated_width_keeps_high_order_bytes() {
        // 1.25 = 0x3FF4 0000 0000 0000 : les 3 octets de poids fort suffisent.
        let le = [0x00, 0xF4, 0x3F];
        assert_eq!(
            row(&le, Endian::Little).num(&num_col(3)),
            NumValue::Value(1.25)
        );
        let be = [0x3F, 0xF4, 0x00];
        assert_eq!(
            row(&be, Endian::Big).num(&num_col(3)),
            NumValue::Value(1.25)
        );
    }

    #[test]
    fn special_missing_codes_unix_little_endian() {
        let m = |tag: u8| [0, 0, 0, 0, 0, tag, 0xFF, 0xFF];
        let col = num_col(8);
        assert_eq!(
            row(&m(0xFE), Endian::Little).num(&col),
            NumValue::Missing(MissingKind::Dot)
        );
        assert_eq!(
            row(&m(0xFF), Endian::Little).num(&col),
            NumValue::Missing(MissingKind::Underscore)
        );
        assert_eq!(
            row(&m(0xFD), Endian::Little).num(&col),
            NumValue::Missing(MissingKind::Letter(b'A'))
        );
        assert_eq!(
            row(&m(0xE4), Endian::Little).num(&col),
            NumValue::Missing(MissingKind::Letter(b'Z'))
        );
        // Windows (0xD1) et NaN IEEE ordinaire : lus comme « . ».
        assert_eq!(
            row(&m(0xD1), Endian::Little).num(&col),
            NumValue::Missing(MissingKind::Dot)
        );
    }

    #[test]
    fn missing_big_endian_is_the_mirror_pattern() {
        let col = num_col(8);
        let dot = [0xFF, 0xFF, 0xFE, 0, 0, 0, 0, 0];
        assert_eq!(
            row(&dot, Endian::Big).num(&col),
            NumValue::Missing(MissingKind::Dot)
        );
    }

    #[test]
    fn char_field_trims_trailing_spaces_and_empties_all_nul() {
        let col = Column {
            kind: ColumnKind::Char { length: 6 },
            ..num_col(8)
        };
        assert_eq!(row(b"ab  c ", Endian::Little).char_bytes(&col), b"ab  c");
        assert_eq!(row(b"      ", Endian::Little).char_bytes(&col), b"");
        assert_eq!(row(&[0u8; 6], Endian::Little).char_bytes(&col), b"");
        assert_eq!(
            row(b"a\0\0\0\0\0", Endian::Little).char_bytes(&col),
            b"a\0\0\0\0\0"
        );
    }

    #[test]
    fn column_outside_the_row_reads_as_empty() {
        let col = Column {
            offset: 100,
            ..num_col(8)
        };
        assert!(row(&[0u8; 8], Endian::Little).bytes(&col).is_empty());
    }
}

//! Métadonnées de la table et balayage des sous-en-têtes (`10` §4 et §6).

use crate::bytes::{slice, to_usize, Layout};
use crate::encoding::{encoding_for_code, trim_text};
use crate::error::{Error, Result};
use crate::formats::{classify_format, LogicalType, StorageKind};
use crate::header::FileHeader;
use crate::limits::Limits;
use crate::page::{PageHeader, PageKind};
use crate::report::{ReadReport, Strictness};

const SIG_ROW_SIZE: u32 = 0xF7F7_F7F7;
const SIG_COLUMN_SIZE: u32 = 0xF6F6_F6F6;
const SIG_COLUMN_TEXT: u32 = 0xFFFF_FFFD;
const SIG_COLUMN_NAME: u32 = 0xFFFF_FFFF;
const SIG_COLUMN_ATTRS: u32 = 0xFFFF_FFFC;
const SIG_FORMAT_LABEL: u32 = 0xFFFF_FBFE;

/// Signatures de sous-en-têtes de métadonnées : une ligne brute rangée comme sous-en-tête n'en porte aucune.
pub(crate) const KNOWN_SIGNATURES: [u32; 9] = [
    SIG_ROW_SIZE,
    SIG_COLUMN_SIZE,
    0xFFFF_FC00,
    SIG_COLUMN_TEXT,
    SIG_COLUMN_NAME,
    SIG_COLUMN_ATTRS,
    SIG_FORMAT_LABEL,
    0xFFFF_FFFE,
    0xFFFF_FFFA,
];

/// Compression des lignes (`15` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Lignes brutes.
    None,
    /// RLE, littéral `SASYZCRL` (`COMPRESS=CHAR`).
    Char,
    /// RDC, littéral `SASYZCR2` (`COMPRESS=BINARY`).
    Binary,
}

/// Type de stockage d'une colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// Numérique stocké sur 1 à 8 octets (3 à 8 observés).
    Numeric {
        /// Longueur de stockage.
        length: u8,
    },
    /// Caractère.
    Char {
        /// Longueur de stockage en octets.
        length: u32,
    },
}

/// Format ou informat : nom, largeur, décimales.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SasFormat {
    /// Nom sans largeur ni point (`DATE`, `$CHAR`, `BEST`) ; vide si seule la largeur est donnée.
    pub name: String,
    /// Largeur.
    pub width: u16,
    /// Décimales.
    pub decimals: u16,
}

/// Une colonne de la table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Rang de création (0-based).
    pub index: usize,
    /// Nom.
    pub name: String,
    /// Label, s'il existe.
    pub label: Option<String>,
    /// Type de stockage.
    pub kind: ColumnKind,
    /// Position de la valeur dans la ligne.
    pub offset: u32,
    /// Format de sortie.
    pub format: Option<SasFormat>,
    /// Informat.
    pub informat: Option<SasFormat>,
    /// Bit 0 de `name_flags` : colonne indexée.
    pub indexed: bool,
    /// Type logique déduit du format (puis de l'informat) et du type de stockage (`30` §1).
    pub logical: LogicalType,
}

impl Column {
    /// Longueur de stockage en octets.
    pub fn width(&self) -> usize {
        match self.kind {
            ColumnKind::Numeric { length } => usize::from(length),
            ColumnKind::Char { length } => usize::try_from(length).unwrap_or(usize::MAX),
        }
    }
}

/// Métadonnées complètes de la table.
#[derive(Debug, Clone, PartialEq)]
pub struct TableMeta {
    /// En-tête de fichier.
    pub header: FileHeader,
    /// Lignes physiques (supprimées comprises).
    pub row_count: u64,
    /// Lignes supprimées.
    pub deleted_count: u64,
    /// Longueur d'une ligne.
    pub row_length: u32,
    /// Compression des lignes.
    pub compression: Compression,
    /// Colonnes, dans l'ordre de création.
    pub columns: Vec<Column>,
}

impl TableMeta {
    /// Lignes visibles : `row_count - deleted_count`.
    pub fn visible_rows(&self) -> u64 {
        self.row_count.saturating_sub(self.deleted_count)
    }
}

/// Résultat d'un appel à [`MetaScanner::feed_page`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanProgress {
    /// Les métadonnées peuvent continuer sur la page suivante.
    NeedMore,
    /// Première page de données pures atteinte : les métadonnées sont complètes.
    Done,
}

#[derive(Debug, Clone, Copy)]
struct TextRef {
    block: u16,
    offset: u16,
    len: u16,
}

#[derive(Debug, Clone, Copy)]
struct Attr {
    offset: u64,
    width: u32,
    flags: u16,
    kind: u8,
}

#[derive(Debug, Clone, Copy)]
struct FormatEntry {
    format_width: u16,
    format_decimals: u16,
    informat_width: u16,
    informat_decimals: u16,
    informat: TextRef,
    format: TextRef,
    label: TextRef,
}

#[derive(Debug, Clone, Copy)]
struct RowSize {
    row_length: u64,
    row_count: u64,
    deleted_count: u64,
}

/// Balayage des pages de métadonnées, sans I/O.
#[derive(Debug)]
pub struct MetaScanner {
    header: FileHeader,
    layout: Layout,
    limits: Limits,
    report: ReadReport,
    texts: Vec<Vec<u8>>,
    text_total: usize,
    names: Vec<TextRef>,
    attrs: Vec<Attr>,
    formats: Vec<FormatEntry>,
    row_size: Option<RowSize>,
    column_count: Option<u64>,
    encoding_override: Option<&'static encoding_rs::Encoding>,
}

fn text_ref(layout: Layout, buf: &[u8], off: usize) -> Option<TextRef> {
    Some(TextRef {
        block: layout.endian.u16(buf, off)?,
        offset: layout.endian.u16(buf, off.checked_add(2)?)?,
        len: layout.endian.u16(buf, off.checked_add(4)?)?,
    })
}

/// Signature de 32 bits d'un sous-en-tête (`10` §4.1) ; `None` si le mot n'a pas la forme attendue.
pub(crate) fn signature(layout: Layout, sh: &[u8]) -> Option<u32> {
    use crate::bytes::Endian;
    if !layout.w64 {
        return layout.endian.u32(sh, 0);
    }
    let head = slice(sh, 0, 4)?;
    let tail = slice(sh, 4, 4)?;
    match layout.endian {
        Endian::Little => {
            let ext = tail.iter().all(|&b| b == 0) || tail.iter().all(|&b| b == 0xFF);
            ext.then(|| layout.endian.u32(sh, 0)).flatten()
        }
        Endian::Big => {
            if head == [0xF7; 4] || head == [0xF6; 4] {
                layout.endian.u32(sh, 0)
            } else {
                layout.endian.u32(sh, 4)
            }
        }
    }
}

impl MetaScanner {
    /// Prépare le balayage pour un en-tête déjà analysé.
    pub fn new(header: &FileHeader, limits: &Limits, strictness: Strictness) -> Self {
        Self {
            header: header.clone(),
            layout: header.layout(),
            limits: limits.clone(),
            report: ReadReport::new(strictness),
            texts: Vec::new(),
            text_total: 0,
            names: Vec::new(),
            attrs: Vec::new(),
            formats: Vec::new(),
            row_size: None,
            column_count: None,
            encoding_override: None,
        }
    }

    /// Impose l'encodage de décodage des noms, formats et labels (priorité sur l'octet 70, U22).
    #[must_use]
    pub fn with_encoding(mut self, encoding: &'static encoding_rs::Encoding) -> Self {
        self.encoding_override = Some(encoding);
        self
    }

    /// Avertissements collectés jusqu'ici.
    pub fn report(&self) -> &ReadReport {
        &self.report
    }

    /// Traite la page `page_index` (exactement `page_length` octets).
    ///
    /// # Errors
    /// `Truncated`, `CorruptPage`, `LimitExceeded` si la page ou un sous-en-tête est invalide.
    pub fn feed_page(&mut self, page_index: u64, page: &[u8]) -> Result<ScanProgress> {
        let ph = PageHeader::parse(&self.header, page_index, page, &mut self.report)?;
        match ph.kind {
            PageKind::Data => return Ok(ScanProgress::Done),
            PageKind::Meta | PageKind::Mixed | PageKind::Amd => {}
            PageKind::Meta2 => self.report.warn(
                "U20",
                Some(page_index),
                "page 0x4000 : sous-en-têtes lus sans preuve",
            )?,
            // La page 0x9000 suit la première page qui contient des lignes : les métadonnées sont complètes.
            PageKind::Compressed => return Ok(ScanProgress::Done),
            PageKind::Other => return Ok(ScanProgress::NeedMore),
        }
        self.scan_pointers(page_index, page, usize::from(ph.subheader_count))?;
        Ok(ScanProgress::NeedMore)
    }

    fn scan_pointers(&mut self, page_index: u64, page: &[u8], count: usize) -> Result<()> {
        let l = self.layout;
        let (w, ps) = (l.w(), l.ps());
        let w2 = w.saturating_mul(2);
        let base = l.po().saturating_add(8);
        let corrupt = Error::CorruptPage {
            page: page_index,
            field: "pointeur de sous-en-tête",
        };
        for i in 0..count {
            let q = base.saturating_add(i.saturating_mul(ps));
            let (Some(off), Some(len), Some(&comp)) = (
                l.word(page, q),
                l.word(page, q.saturating_add(w)),
                page.get(q.saturating_add(w2)),
            ) else {
                return Err(corrupt);
            };
            // pointeur terminal (longueur 0), zone libre (comp 1) ; lignes compressées et autres : jalon J2.
            if len == 0 || comp != 0 {
                continue;
            }
            let sh = to_usize(off)
                .zip(to_usize(len))
                .and_then(|(o, n)| slice(page, o, n))
                .ok_or(Error::CorruptPage {
                    page: page_index,
                    field: "sous-en-tête hors page",
                })?;
            if sh.len() < w2 {
                continue;
            }
            self.handle(page_index, sh)?;
        }
        Ok(())
    }

    fn handle(&mut self, page_index: u64, sh: &[u8]) -> Result<()> {
        let l = self.layout;
        let (w, w2) = (l.w(), l.w().saturating_mul(2));
        let Some(sig) = signature(l, sh) else {
            return Ok(());
        };
        let bad = || Error::CorruptSubheader {
            page: page_index,
            signature: sig,
        };
        match sig {
            SIG_ROW_SIZE => {
                let word = |k: usize| l.word(sh, w.saturating_mul(k));
                let (Some(row_length), Some(row_count), Some(deleted_count)) =
                    (word(5), word(6), word(7))
                else {
                    return Err(bad());
                };
                if row_length > u64::from(self.limits.max_row_length) {
                    return Err(Error::LimitExceeded {
                        what: "row_length",
                        value: row_length,
                        limit: u64::from(self.limits.max_row_length),
                    });
                }
                self.row_size = Some(RowSize {
                    row_length,
                    row_count,
                    deleted_count,
                });
            }
            SIG_COLUMN_SIZE => {
                let n = l.word(sh, w).ok_or_else(bad)?;
                if n > u64::from(self.limits.max_columns) {
                    return Err(Error::LimitExceeded {
                        what: "column_count",
                        value: n,
                        limit: u64::from(self.limits.max_columns),
                    });
                }
                self.column_count = Some(n);
            }
            SIG_COLUMN_TEXT => {
                // Le bloc commence à l'offset `w` (champ `text_size`, puis le texte). Les références peuvent
                // dépasser `text_size` dans des fichiers anciens (`pandas/many_columns`, release 9.0101M0) :
                // le bloc retenu va donc jusqu'à la fin du sous-en-tête.
                l.endian.u16(sh, w).ok_or_else(bad)?;
                let block = slice(sh, w, sh.len().saturating_sub(w)).ok_or_else(bad)?;
                self.text_total = self.text_total.saturating_add(block.len());
                if self.text_total > self.limits.max_text_bytes {
                    return Err(Error::LimitExceeded {
                        what: "texte des colonnes",
                        value: self.text_total as u64,
                        limit: self.limits.max_text_bytes as u64,
                    });
                }
                self.texts.push(block.to_vec());
            }
            SIG_COLUMN_NAME => {
                let n = sh
                    .len()
                    .checked_sub(w2.saturating_add(12))
                    .ok_or_else(bad)?
                    / 8;
                self.check_column_room(self.names.len(), n)?;
                for j in 0..n {
                    let x = w.saturating_add(8).saturating_add(j.saturating_mul(8));
                    self.names.push(text_ref(l, sh, x).ok_or_else(bad)?);
                }
            }
            SIG_COLUMN_ATTRS => {
                let es = w.saturating_add(8);
                let n = sh
                    .len()
                    .checked_sub(w2.saturating_add(12))
                    .ok_or_else(bad)?
                    .checked_div(es)
                    .unwrap_or(0);
                self.check_column_room(self.attrs.len(), n)?;
                for j in 0..n {
                    let x = w.saturating_add(8).saturating_add(j.saturating_mul(es));
                    let width_at = x.saturating_add(w);
                    self.attrs.push(Attr {
                        offset: l.word(sh, x).ok_or_else(bad)?,
                        width: l.endian.u32(sh, width_at).ok_or_else(bad)?,
                        flags: l
                            .endian
                            .u16(sh, width_at.saturating_add(4))
                            .ok_or_else(bad)?,
                        kind: *sh.get(width_at.saturating_add(6)).ok_or_else(bad)?,
                    });
                }
            }
            SIG_FORMAT_LABEL => {
                self.check_column_room(self.formats.len(), 1)?;
                if !l.w64 && self.formats.is_empty() {
                    self.report.warn(
                        "U24",
                        Some(page_index),
                        "offsets 32 bits de FormatAndLabel peu vérifiés",
                    )?;
                }
                let entry = Self::parse_format(l, sh).ok_or_else(bad)?;
                self.formats.push(entry);
            }
            _ => {}
        }
        Ok(())
    }

    fn check_column_room(&self, have: usize, add: usize) -> Result<()> {
        let total = have.saturating_add(add);
        if total > usize::try_from(self.limits.max_columns).unwrap_or(usize::MAX) {
            return Err(Error::LimitExceeded {
                what: "column_count",
                value: total as u64,
                limit: u64::from(self.limits.max_columns),
            });
        }
        Ok(())
    }

    fn parse_format(l: Layout, sh: &[u8]) -> Option<FormatEntry> {
        // (largeur de format, décimales, largeur d'informat, décimales, réf. informat, réf. format, réf. label)
        let o: [usize; 7] = if l.w64 {
            [0x18, 0x1A, 0x1C, 0x1E, 0x28, 0x2E, 0x34]
        } else {
            [0x0C, 0x0E, 0x10, 0x12, 0x1C, 0x22, 0x28]
        };
        let [fw, fd, iw, id, ir, fr, lr] = o;
        Some(FormatEntry {
            format_width: l.endian.u16(sh, fw)?,
            format_decimals: l.endian.u16(sh, fd)?,
            informat_width: l.endian.u16(sh, iw)?,
            informat_decimals: l.endian.u16(sh, id)?,
            informat: text_ref(l, sh, ir)?,
            format: text_ref(l, sh, fr)?,
            label: text_ref(l, sh, lr)?,
        })
    }

    fn resolve(&self, r: TextRef) -> Result<String> {
        if r.len == 0 {
            return Ok(String::new());
        }
        let bad = Error::CorruptMetadata {
            what: "référence de texte hors bloc",
        };
        let block = self.texts.get(usize::from(r.block)).ok_or(bad.clone())?;
        let raw = slice(block, usize::from(r.offset), usize::from(r.len)).ok_or(bad)?;
        Ok(self.decode(raw))
    }

    fn decode(&self, raw: &[u8]) -> String {
        match self
            .encoding_override
            .or_else(|| encoding_for_code(self.header.encoding_code))
        {
            Some(enc) => trim_text(&enc.decode_without_bom_handling(raw).0).to_owned(),
            None => {
                trim_text(&encoding_rs::WINDOWS_1252.decode_without_bom_handling(raw).0).to_owned()
            }
        }
    }

    fn sas_format(&self, name: TextRef, width: u16, decimals: u16) -> Result<Option<SasFormat>> {
        let name = self.resolve(name)?;
        Ok(
            (!name.is_empty() || width != 0 || decimals != 0).then_some(SasFormat {
                name,
                width,
                decimals,
            }),
        )
    }

    /// Termine le balayage et rend les métadonnées complètes avec les avertissements.
    ///
    /// # Errors
    /// `CorruptMetadata` si RowSize est absent, si les nombres de colonnes divergent ou si une référence
    /// de texte sort de son bloc.
    pub fn finish(mut self) -> Result<(TableMeta, ReadReport)> {
        let Some(rs) = self.row_size else {
            return Err(Error::CorruptMetadata {
                what: "RowSize absent",
            });
        };
        if self
            .column_count
            .is_some_and(|n| n != self.attrs.len() as u64)
        {
            return Err(Error::CorruptMetadata {
                what: "nombre de ColumnAttributes différent de ColumnSize",
            });
        }
        let row_length = u32::try_from(rs.row_length)
            .map_err(|_| Error::CorruptMetadata { what: "row_length" })?;
        // U22 : code d'encodage hors du sous-ensemble connu.
        if self.encoding_override.is_none()
            && encoding_for_code(self.header.encoding_code).is_none()
        {
            self.report.warn(
                "U22",
                None,
                "code d'encodage hors du sous-ensemble de J1 : lu comme windows-1252",
            )?;
        }
        let mut columns = Vec::with_capacity(self.attrs.len());
        for (j, a) in self.attrs.iter().enumerate() {
            let kind = match a.kind {
                1 => {
                    let length = u8::try_from(a.width).ok().filter(|n| (1..=8).contains(n));
                    let length = length.ok_or(Error::CorruptMetadata {
                        what: "largeur numérique hors 1..8",
                    })?;
                    if length < 3 {
                        self.report.warn(
                            "U21",
                            None,
                            "numérique de largeur 1 ou 2 : lu comme 3 à 7",
                        )?;
                    }
                    ColumnKind::Numeric { length }
                }
                2 => ColumnKind::Char { length: a.width },
                _ => {
                    return Err(Error::CorruptMetadata {
                        what: "type de colonne inconnu",
                    })
                }
            };
            let offset = u32::try_from(a.offset).map_err(|_| Error::CorruptMetadata {
                what: "offset de colonne",
            })?;
            let end = u64::from(offset).saturating_add(match kind {
                ColumnKind::Numeric { length } => u64::from(length),
                ColumnKind::Char { length } => u64::from(length),
            });
            if end > rs.row_length {
                return Err(Error::CorruptMetadata {
                    what: "colonne hors de la ligne",
                });
            }
            let name = match self.names.get(j) {
                Some(&r) => self.resolve(r)?,
                None => String::new(),
            };
            let (format, informat, label) = match self.formats.get(j) {
                Some(f) => {
                    let label = self.resolve(f.label)?;
                    (
                        self.sas_format(f.format, f.format_width, f.format_decimals)?,
                        self.sas_format(f.informat, f.informat_width, f.informat_decimals)?,
                        (!label.is_empty()).then_some(label),
                    )
                }
                None => (None, None, None),
            };
            let storage = match kind {
                ColumnKind::Numeric { .. } => StorageKind::Numeric,
                ColumnKind::Char { .. } => StorageKind::Char,
            };
            let logical = {
                let named = |f: &Option<SasFormat>| {
                    f.as_ref()
                        .filter(|f| !f.name.is_empty())
                        .map(|f| f.name.clone())
                };
                classify_format(
                    &named(&format)
                        .or_else(|| named(&informat))
                        .unwrap_or_default(),
                    storage,
                )
            };
            columns.push(Column {
                index: j,
                name,
                label,
                kind,
                offset,
                format,
                informat,
                indexed: a.flags & 1 != 0,
                logical,
            });
        }
        let compression = self
            .texts
            .first()
            .map_or(Compression::None, |b| crate::compress::detect(b));
        let meta = TableMeta {
            header: self.header,
            row_count: rs.row_count,
            deleted_count: rs.deleted_count,
            row_length,
            compression,
            columns,
        };
        Ok((meta, self.report))
    }
}

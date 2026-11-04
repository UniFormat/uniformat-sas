//! `FileBuilder` : construit les pages d'un fichier `.sas7bdat` ligne par ligne, sans entrée/sortie (`20` §4).
//!
//! Les pages de données terminées sortent par [`FileBuilder::take_pages`] ; la page 0 (métadonnées, première
//! lignes, `RowSize` dépendant des totaux), la page marqueur d'une table compressée et l'en-tête sortent à
//! [`FileBuilder::finish`], car leur contenu dépend du nombre total de lignes.

use std::borrow::Cow;

use super::meta::{
    add, build, ceil8, layout, mul, sub, to_u16, to_u64, Buf, ColLayout, Meta, MAX_CHAR_LENGTH,
    MAX_COLUMNS, MAX_LABEL_BYTES, MAX_NAME_BYTES, MAX_PAGE, MIN_COMPRESSED_ROW, MIN_PAGE, SIG4,
};
use super::types::{
    CellProblem, CellValue, SchemaError, Unrepresentable, WriteColumn, WriteError, WriteKind,
    WriteOptions, WriteReport, WriteSchema,
};
use crate::compress::{rdc_compress, rle_compress};
use crate::{Compression, MissingKind};

/// Octets de l'en-tête de page.
const PAGE_HEADER: usize = 40;
/// Taille d'un pointeur de sous-en-tête.
const PTR: usize = 24;
/// Nombre maximal de pages portant des lignes dans une table compressée : la page marqueur tient
/// `n` enregistrements de 18 octets dans `[A − 18 n, A)`, `A = min(P, 32 767)`.
const MAX_MARKER_PAGES: usize = 1_800;

const MAGIC: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xc2, 0xea, 0x81, 0x60, 0xb3, 0x14, 0x11, 0xcf, 0xbd, 0x92,
    0x08, 0x00, 0x09, 0xc7, 0x31, 0x8c, 0x18, 0x1f, 0x10, 0x11,
];

/// Résultat de [`FileBuilder::finish`].
#[derive(Debug, Clone)]
pub struct Finished {
    /// En-tête de fichier (`page_length` octets), à écrire à l'offset 0.
    pub header: Vec<u8>,
    /// Pages dont le contenu dépend des totaux, avec leur rang (la page 0, et la page marqueur d'une table
    /// compressée) : à écrire à `page_length × (1 + rang)`.
    pub fixed_pages: Vec<(u64, Vec<u8>)>,
    /// Dernières pages de lignes, terminées par `finish` (même format que [`FileBuilder::take_pages`]).
    pub tail_pages: Vec<(u64, Vec<u8>)>,
    /// Nombre de pages (marqueur compris).
    pub page_count: u64,
    /// Taille de page finalement utilisée (peut dépasser la taille demandée, `WriteOptions::grow_page`).
    pub page_length: u32,
    /// Écarts comptés.
    pub report: WriteReport,
}

#[derive(Debug, Clone, Default)]
struct CPage {
    ptrs: Vec<(u8, usize)>,
    data: Vec<Vec<u8>>,
    used: usize,
    meta: bool,
}

#[derive(Debug, Clone)]
struct Plain {
    cap: usize,
    rmax: usize,
    start: usize,
    minoff: usize,
    page0_rows: Vec<u8>,
    cur: Vec<u8>,
    cur_rows: usize,
    data_pages: usize,
}

#[derive(Debug, Clone)]
struct Stat {
    logical: usize,
    rows: usize,
    blocks: usize,
}

#[derive(Debug, Clone)]
struct Comp {
    algo: Compression,
    cur: CPage,
    cur_logical: usize,
    page0: Option<CPage>,
    first: Option<usize>,
    stats: Vec<Stat>,
    cum_rows: usize,
}

#[derive(Debug, Clone)]
enum Body {
    Plain(Plain),
    Comp(Comp),
}

/// Constructeur de fichier. Voir le module.
#[derive(Debug, Clone)]
pub struct FileBuilder {
    opts: WriteOptions,
    columns: Vec<WriteColumn>,
    cols: Vec<ColLayout>,
    row_len: usize,
    page_len: usize,
    meta: Meta,
    rows: u64,
    pending: Vec<(u64, Vec<u8>)>,
    report: WriteReport,
    scratch: Vec<u8>,
    body: Body,
}

fn cap_for(avail: usize, rl: usize) -> usize {
    let Some(mut c) = avail.checked_div(rl) else {
        return 0;
    };
    while c > 0 && add(mul(c, rl), add(c, 7) / 8) > avail {
        c = sub(c, 1);
    }
    c
}

fn validate(schema: &WriteSchema, opts: &WriteOptions) -> Result<WriteReport, SchemaError> {
    let mut report = WriteReport::default();
    let n = schema.columns.len();
    if n == 0 {
        return Err(SchemaError::NoColumns);
    }
    if n > MAX_COLUMNS {
        return Err(SchemaError::TooManyColumns {
            columns: n,
            page_length: opts.page_length,
            max_for_page: MAX_COLUMNS,
        });
    }
    if !(MIN_PAGE..=MAX_PAGE).contains(&opts.page_length) {
        return Err(SchemaError::PageLength {
            page_length: opts.page_length,
        });
    }
    let mut seen: Vec<&str> = Vec::with_capacity(n);
    let mut lower: Vec<String> = Vec::with_capacity(n);
    for (i, c) in schema.columns.iter().enumerate() {
        if c.name.is_empty() {
            return Err(SchemaError::EmptyName { index: i });
        }
        let name = opts
            .encoding
            .encode(&c.name)
            .ok_or_else(|| SchemaError::UnencodableText {
                name: c.name.clone(),
            })?;
        if name.len() > MAX_NAME_BYTES {
            return Err(SchemaError::NameTooLong {
                name: c.name.clone(),
                bytes: name.len(),
                max: MAX_NAME_BYTES,
            });
        }
        if seen.contains(&c.name.as_str()) {
            return Err(SchemaError::DuplicateName {
                name: c.name.clone(),
            });
        }
        let low = c.name.to_lowercase();
        if lower.contains(&low) {
            report
                .warnings
                .push(format!("name_case_collision : {}", c.name));
        }
        seen.push(&c.name);
        lower.push(low);
        let label = opts
            .encoding
            .encode(&c.label)
            .ok_or_else(|| SchemaError::UnencodableText {
                name: c.name.clone(),
            })?;
        if label.len() > MAX_LABEL_BYTES {
            return Err(SchemaError::LabelTooLong {
                name: c.name.clone(),
                bytes: label.len(),
                max: MAX_LABEL_BYTES,
            });
        }
        match c.kind {
            WriteKind::Numeric { length } if !(3..=8).contains(&length) => {
                return Err(SchemaError::NumericLength {
                    name: c.name.clone(),
                    length,
                });
            }
            WriteKind::Char { length } if length == 0 || length > MAX_CHAR_LENGTH => {
                return Err(SchemaError::CharLength {
                    name: c.name.clone(),
                    length,
                    max: MAX_CHAR_LENGTH,
                });
            }
            _ => {}
        }
    }
    Ok(report)
}

/// Taille de page minimale pour les métadonnées et une ligne (`20` §6.1).
fn min_page(meta_total: usize, n_sub: usize, row_len: usize, compressed: bool) -> usize {
    let sc = add(n_sub, 1);
    if compressed {
        // métadonnées + pointeurs sur la page 0 ; une ligne compressée (au pire un peu plus longue) sur une page
        let meta_need = add(add(meta_total, PAGE_HEADER), mul(PTR, sc));
        let row_need = add(
            add(PAGE_HEADER, mul(PTR, 2)),
            add(add(row_len, row_len / 7), 16),
        );
        meta_need.max(row_need)
    } else {
        let meta_need = add(meta_total, ceil8(add(PAGE_HEADER, mul(PTR, sc))));
        let row_need = add(add(row_len, 1), PAGE_HEADER);
        meta_need.max(row_need)
    }
}

fn sas_code(kind: MissingKind) -> u8 {
    match kind {
        MissingKind::Dot => 0xFE,
        MissingKind::Underscore => 0xFF,
        MissingKind::Letter(c) => 0xFDu8.wrapping_sub(c.to_ascii_uppercase().wrapping_sub(b'A')),
    }
}

fn missing_raw(code: u8) -> [u8; 8] {
    [0, 0, 0, 0, 0, code, 0xFF, 0xFF]
}

impl FileBuilder {
    /// Valide le schéma et prépare l'écriture : aucune limite de schéma n'est découverte après le premier octet.
    ///
    /// # Errors
    /// [`SchemaError`] : nom, label, longueur, nombre de colonnes, taille de page.
    pub fn new(schema: &WriteSchema, mut opts: WriteOptions) -> Result<Self, SchemaError> {
        let mut report = validate(schema, &opts)?;
        let total_len: usize = schema.columns.iter().map(WriteColumn::length).sum();
        let mut compression = opts.compression;
        if compression != Compression::None && total_len < MIN_COMPRESSED_ROW {
            report.warnings.push(format!(
                "compression_disabled_small_rows : lignes de {total_len} octets (< {MIN_COMPRESSED_ROW})"
            ));
            compression = Compression::None;
        }
        let compressed = compression != Compression::None;
        let (cols, row_len) = layout(&schema.columns, compressed);
        let mut meta = build(
            schema,
            &cols,
            row_len,
            compressed.then_some(compression),
            &opts,
        )?;
        let need = min_page(meta.total, meta.subs.len(), row_len, compressed);
        let requested = usize::try_from(opts.page_length).unwrap_or(usize::MAX);
        let page_len = if need <= requested {
            requested
        } else if opts.grow_page {
            let grown = need.next_power_of_two().max(requested);
            if grown > usize::try_from(MAX_PAGE).unwrap_or(usize::MAX) {
                return Err(too_big(schema, &opts, row_len, need));
            }
            report
                .warnings
                .push(format!("page_length_grown : {requested} -> {grown} octets"));
            grown
        } else {
            return Err(too_big(schema, &opts, row_len, need));
        };
        opts.page_length = u32::try_from(page_len).unwrap_or(MAX_PAGE);
        if let Some(rs) = meta.subs.first_mut() {
            if let Some(dst) = rs.data.get_mut(104..112) {
                dst.copy_from_slice(&u64::from(opts.page_length).to_le_bytes());
            }
        }
        let n_sub = meta.subs.len();
        let body = if compressed {
            let page0 = CPage {
                meta: true,
                used: meta.total,
                ..CPage::default()
            };
            Body::Comp(Comp {
                algo: compression,
                cur: page0,
                cur_logical: 0,
                page0: None,
                first: None,
                stats: Vec::new(),
                cum_rows: 0,
            })
        } else {
            let sc = add(n_sub, 1);
            let start = ceil8(add(PAGE_HEADER, mul(PTR, sc)));
            let minoff = sub(page_len, meta.total);
            let cap = cap_for(sub(minoff, start), row_len);
            let rmax = cap_for(sub(page_len, PAGE_HEADER), row_len);
            Body::Plain(Plain {
                cap,
                rmax,
                start,
                minoff,
                page0_rows: Vec::new(),
                cur: Vec::new(),
                cur_rows: 0,
                data_pages: 0,
            })
        };
        Ok(Self {
            opts,
            columns: schema.columns.clone(),
            cols,
            row_len,
            page_len,
            meta,
            rows: 0,
            pending: Vec::new(),
            report,
            scratch: Vec::new(),
            body,
        })
    }

    /// Taille de page utilisée.
    #[must_use]
    pub fn page_length(&self) -> u32 {
        self.opts.page_length
    }

    /// Compression effectivement appliquée (une table de moins de 16 octets par ligne n'est pas compressée).
    #[must_use]
    pub fn compression(&self) -> Compression {
        match &self.body {
            Body::Plain(_) => Compression::None,
            Body::Comp(c) => c.algo,
        }
    }

    /// Longueur d'une ligne brute, en octets.
    #[must_use]
    pub fn row_length(&self) -> usize {
        self.row_len
    }

    /// Lignes reçues.
    #[must_use]
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Écarts comptés jusqu'ici.
    #[must_use]
    pub fn report(&self) -> &WriteReport {
        &self.report
    }

    /// Compteurs modifiables par la couche appelante (entiers arrondis, avertissements…).
    pub fn report_mut(&mut self) -> &mut WriteReport {
        &mut self.report
    }

    /// Pages terminées depuis le dernier appel : `(rang de page, octets)`, rang 0 = première page après l'en-tête.
    pub fn take_pages(&mut self) -> Vec<(u64, Vec<u8>)> {
        std::mem::take(&mut self.pending)
    }

    fn problem(&self, column: usize, p: CellProblem) -> WriteError {
        WriteError::Cell {
            column,
            row: self.rows,
            problem: p,
        }
    }

    /// Ajoute une ligne.
    ///
    /// # Errors
    /// Forme de la ligne, valeur non représentable (selon `on_unrepresentable`), ligne compressée trop longue.
    pub fn push_row(&mut self, cells: &[CellValue<'_>]) -> Result<(), WriteError> {
        if cells.len() != self.columns.len() {
            return Err(WriteError::RowShape {
                row: self.rows,
                expected: self.columns.len(),
                got: cells.len(),
            });
        }
        let mut row = std::mem::take(&mut self.scratch);
        row.clear();
        row.resize(self.row_len, 0);
        let res = self.encode_row(cells, &mut row);
        let res = res.and_then(|()| self.store_row(&row));
        self.scratch = row;
        res?;
        self.rows = self.rows.saturating_add(1);
        self.report.rows = self.rows;
        Ok(())
    }

    fn encode_row(&mut self, cells: &[CellValue<'_>], row: &mut [u8]) -> Result<(), WriteError> {
        let policy = self.opts.on_unrepresentable;
        for (j, cell) in cells.iter().enumerate() {
            let (Some(col), Some(spec)) = (self.cols.get(j).copied(), self.columns.get(j)) else {
                continue;
            };
            let kind = spec.kind;
            let end = add(col.offset, col.length);
            if let WriteKind::Numeric { length } = kind {
                let raw = match *cell {
                    CellValue::Num(x) if x.is_nan() => missing_raw(0xFE),
                    CellValue::Num(x) if x.is_infinite() => {
                        if policy == Unrepresentable::Replace {
                            self.report.replaced_values =
                                self.report.replaced_values.saturating_add(1);
                            missing_raw(0xFE)
                        } else {
                            return Err(self.problem(j, CellProblem::Infinite));
                        }
                    }
                    CellValue::Num(x) => x.to_le_bytes(),
                    CellValue::Missing(m) => missing_raw(sas_code(m)),
                    CellValue::Null => missing_raw(0xFE),
                    CellValue::Text(_) | CellValue::Bytes(_) => {
                        return Err(self.problem(j, CellProblem::WrongType));
                    }
                };
                let drop = sub(8, usize::from(length));
                if drop > 0
                    && raw
                        .get(..drop)
                        .is_some_and(|lost| lost.iter().any(|&b| b != 0))
                {
                    // les octets de poids faible sont perdus (`raw[8-k:]` ne garde que les octets de poids fort)
                    if matches!(cell, CellValue::Num(_)) {
                        if policy == Unrepresentable::Error {
                            return Err(self.problem(j, CellProblem::Narrowed));
                        }
                        self.report.narrowed_values = self.report.narrowed_values.saturating_add(1);
                    }
                }
                if let Some(dst) = row.get_mut(col.offset..end) {
                    if let Some(src) = raw.get(drop..) {
                        dst.copy_from_slice(src);
                    }
                }
            } else {
                let (bytes, truncated) = self.text_bytes(j, cell, col.length)?;
                if let Some(dst) = row.get_mut(col.offset..end) {
                    let (head, tail) = dst.split_at_mut(bytes.len().min(dst.len()));
                    head.copy_from_slice(bytes.get(..head.len()).unwrap_or(&[]));
                    tail.fill(b' ');
                }
                if truncated {
                    self.report.truncated_values = self.report.truncated_values.saturating_add(1);
                }
            }
        }
        Ok(())
    }

    /// Octets d'une cellule caractère (au plus `max`) et indicateur de troncature.
    fn text_bytes(
        &mut self,
        j: usize,
        cell: &CellValue<'_>,
        max: usize,
    ) -> Result<(Cow<'static, [u8]>, bool), WriteError> {
        let policy = self.opts.on_unrepresentable;
        match *cell {
            CellValue::Null => Ok((Cow::Borrowed(&[]), false)),
            CellValue::Num(_) | CellValue::Missing(_) => {
                Err(self.problem(j, CellProblem::WrongType))
            }
            CellValue::Bytes(b) => {
                if b.len() <= max {
                    Ok((Cow::Owned(b.to_vec()), false))
                } else if policy == Unrepresentable::Truncate {
                    Ok((Cow::Owned(b.get(..max).unwrap_or(b).to_vec()), true))
                } else {
                    Err(self.problem(
                        j,
                        CellProblem::TextTooLong {
                            bytes: b.len(),
                            max,
                        },
                    ))
                }
            }
            CellValue::Text(s) => {
                let enc = self.opts.encoding;
                let bytes: Cow<'_, [u8]> = match enc.encode(s) {
                    Some(b) => b,
                    None if policy == Unrepresentable::Replace => {
                        let (b, replaced) = enc.encode_lossy(s);
                        self.report.replaced_chars =
                            self.report.replaced_chars.saturating_add(replaced);
                        b
                    }
                    None => return Err(self.problem(j, CellProblem::Unencodable)),
                };
                if bytes.len() <= max {
                    return Ok((Cow::Owned(bytes.into_owned()), false));
                }
                if policy != Unrepresentable::Truncate {
                    return Err(self.problem(
                        j,
                        CellProblem::TextTooLong {
                            bytes: bytes.len(),
                            max,
                        },
                    ));
                }
                // tronque à la limite d'un caractère, jamais au milieu d'un caractère multi-octets
                let mut used = 0usize;
                let mut buf = [0u8; 4];
                for c in s.chars() {
                    let piece = enc.encode_lossy(c.encode_utf8(&mut buf)).0;
                    if add(used, piece.len()) > max {
                        break;
                    }
                    used = add(used, piece.len());
                }
                Ok((Cow::Owned(bytes.get(..used).unwrap_or(&[]).to_vec()), true))
            }
        }
    }

    fn store_row(&mut self, row: &[u8]) -> Result<(), WriteError> {
        match &mut self.body {
            Body::Plain(p) => {
                if sub(
                    p.cap,
                    p.page0_rows.len().checked_div(self.row_len).unwrap_or(0),
                ) > 0
                    && p.cur_rows == 0
                    && p.data_pages == 0
                {
                    p.page0_rows.extend_from_slice(row);
                    return Ok(());
                }
                p.cur.extend_from_slice(row);
                p.cur_rows = add(p.cur_rows, 1);
                if p.cur_rows == p.rmax {
                    let rank = add(p.data_pages, 1);
                    let bytes = plain_data_page(
                        &self.opts,
                        self.page_len,
                        self.row_len,
                        p.rmax,
                        rank,
                        &p.cur,
                        p.cur_rows,
                    );
                    self.pending.push((to_u64(rank), bytes));
                    p.data_pages = rank;
                    p.cur.clear();
                    p.cur_rows = 0;
                }
                Ok(())
            }
            Body::Comp(_) => self.store_compressed(row),
        }
    }

    fn store_compressed(&mut self, row: &[u8]) -> Result<(), WriteError> {
        let n_sub = self.meta.subs.len();
        let page_len = self.page_len;
        let row_idx = self.rows;
        let Body::Comp(c) = &mut self.body else {
            return Ok(());
        };
        let mut comp = Vec::new();
        match c.algo {
            Compression::Binary => rdc_compress(row, &mut comp),
            _ => rle_compress(row, &mut comp),
        }
        // une ligne brute de moins de 16 octets, ou qui commence comme une signature de sous-en-tête, serait prise
        // pour un sous-en-tête : on garde alors le flot, même plus long que la ligne (`20` §4.2 point 4)
        let ambiguous = row.len() < MIN_COMPRESSED_ROW
            || row
                .get(..4)
                .is_some_and(|h| SIG4.iter().any(|s| s.as_slice() == h));
        let (flag, data) = if comp.len() >= row.len() && !ambiguous {
            (0u8, row.to_vec())
        } else {
            (4u8, comp)
        };
        let fits = |p: &CPage, len: usize| -> bool {
            let nptr = add(add(if p.meta { n_sub } else { 0 }, p.ptrs.len()), 2);
            let used = add(add(p.used, len), add(PAGE_HEADER, mul(nptr, PTR)));
            used <= page_len
        };
        if !fits(&c.cur, data.len()) {
            if c.cur.ptrs.is_empty() && !c.cur.meta {
                return Err(WriteError::CompressedRowTooLong {
                    row: row_idx,
                    bytes: data.len(),
                    page_length: u32::try_from(page_len).unwrap_or(u32::MAX),
                });
            }
            // la page courante est complète
            let done = std::mem::take(&mut c.cur);
            let logical = c.cur_logical;
            if logical == 0 {
                c.page0 = Some(done);
            } else {
                let idx = file_index(logical, c.first);
                let bytes = build_cpage(&self.meta, &self.opts, self.page_len, &done, idx);
                self.pending.push((to_u64(idx), bytes));
            }
            c.cur_logical = add(c.cur_logical, 1);
            if !fits(&c.cur, data.len()) {
                return Err(WriteError::CompressedRowTooLong {
                    row: row_idx,
                    bytes: data.len(),
                    page_length: u32::try_from(page_len).unwrap_or(u32::MAX),
                });
            }
        }
        if c.first.is_none() {
            c.first = Some(c.cur_logical);
        }
        if c.stats.last().map(|s| s.logical) != Some(c.cur_logical) {
            if c.stats.len() >= MAX_MARKER_PAGES {
                return Err(WriteError::Unsupported(
                    "table compressée de plus de 1 800 pages de lignes (page marqueur)",
                ));
            }
            c.stats.push(Stat {
                logical: c.cur_logical,
                rows: 0,
                blocks: 0,
            });
        }
        c.cur.ptrs.push((flag, data.len()));
        c.cur.used = add(c.cur.used, data.len());
        c.cur.data.push(data);
        c.cum_rows = add(c.cum_rows, 1);
        if let Some(s) = c.stats.last_mut() {
            s.rows = add(s.rows, 1);
            s.blocks = add(add(if c.cur.meta { n_sub } else { 0 }, c.cur.ptrs.len()), 1);
        }
        Ok(())
    }

    /// Termine le fichier : rend l'en-tête, la page 0 et, pour une table compressée, la page marqueur.
    ///
    /// # Errors
    /// Aucune erreur de données n'est possible ici ; l'erreur ne rend qu'une incohérence interne.
    pub fn finish(mut self) -> Result<Finished, WriteError> {
        let n_sub = self.meta.subs.len();
        let n_rows = self.rows;
        let mut fixed: Vec<(u64, Vec<u8>)> = Vec::new();
        let sc = add(n_sub, 1);
        let sig0 = self.opts.file_code ^ 1;
        let mut rowsize = self
            .meta
            .subs
            .first()
            .map_or_else(Vec::new, |s| s.data.clone());
        let mut rs = Buf(std::mem::take(&mut rowsize));
        rs.u64(48, n_rows);
        rs.u64(776, n_rows);
        rs.u32(0x1B8, sig0);
        let page_count = match std::mem::replace(
            &mut self.body,
            Body::Plain(Plain {
                cap: 0,
                rmax: 0,
                start: 0,
                minoff: 0,
                page0_rows: Vec::new(),
                cur: Vec::new(),
                cur_rows: 0,
                data_pages: 0,
            }),
        ) {
            Body::Plain(mut p) => {
                if p.cur_rows > 0 {
                    let rank = add(p.data_pages, 1);
                    let bytes = plain_data_page(
                        &self.opts,
                        self.page_len,
                        self.row_len,
                        p.rmax,
                        rank,
                        &p.cur,
                        p.cur_rows,
                    );
                    self.pending.push((to_u64(rank), bytes));
                    p.data_pages = rank;
                    p.cur_rows = 0;
                }
                let n0 = p.page0_rows.len().checked_div(self.row_len).unwrap_or(0);
                let npages = add(p.data_pages, 1);
                let rest_rows = sub(usize::try_from(n_rows).unwrap_or(usize::MAX), n0);
                let last_blocks = if p.data_pages == 0 {
                    add(sc, n0)
                } else {
                    sub(rest_rows, mul(sub(npages, 2), p.rmax))
                };
                rs.u64(120, to_u64(p.cap));
                let tail = if usize::try_from(n_rows).unwrap_or(usize::MAX) <= p.cap {
                    add(p.cap, sc)
                } else {
                    p.rmax
                };
                rs.u16(0x2FE, to_u16(tail));
                let locs = [
                    (1u64, 2u64),
                    (1, to_u64(add(self.meta.last_fl, 1))),
                    (1, to_u64(add(sc, 1))),
                    (to_u64(npages), to_u64(last_blocks)),
                    (1, to_u64(add(self.meta.first_fl, 1))),
                ];
                for (k, (pg, v)) in locs.into_iter().enumerate() {
                    rs.u64(add(512, mul(16, k)), pg);
                    rs.u64(add(520, mul(16, k)), v);
                }
                let mut pg = Buf::zeros(self.page_len);
                pg.u32(0, sig0);
                let free = sub(
                    sub(p.minoff, add(p.start, mul(n0, self.row_len))),
                    add(p.cap, 7) / 8,
                );
                pg.u64(24, to_u64(free));
                pg.u16(32, 0x0200);
                pg.u16(34, to_u16(add(sc, n0)));
                pg.u16(36, to_u16(sc));
                let mut off = self.page_len;
                let mut ptrs: Vec<u8> = Vec::with_capacity(mul(sc, PTR));
                for (k, s) in self.meta.subs.iter().enumerate() {
                    off = sub(off, s.data.len());
                    if k == 0 {
                        pg.put(off, &rs.0);
                    } else {
                        pg.put(off, &s.data);
                    }
                    push_ptr(&mut ptrs, off, s.data.len(), 0, s.ptype);
                }
                push_ptr(&mut ptrs, off, 0, 1, 0);
                pg.put(PAGE_HEADER, &ptrs);
                pg.put(p.start, &p.page0_rows);
                fixed.push((0, pg.0));
                to_u64(npages)
            }
            Body::Comp(mut c) => {
                // page courante : page 0 ou dernière page de lignes
                let done = std::mem::take(&mut c.cur);
                if c.cur_logical == 0 {
                    c.page0 = Some(done);
                } else {
                    let idx = file_index(c.cur_logical, c.first);
                    let bytes = build_cpage(&self.meta, &self.opts, self.page_len, &done, idx);
                    self.pending.push((to_u64(idx), bytes));
                }
                let first = c.first.unwrap_or(0);
                let logical_pages = add(c.cur_logical, 1);
                // dernier page portant des lignes, en rang de fichier
                let (last_idx, sc_last) = match c.stats.last() {
                    Some(s) => (file_index(s.logical, c.first), s.blocks),
                    None => (0, add(n_sub, 1)),
                };
                rs.u64(
                    16,
                    to_u64(add(
                        add(usize::try_from(n_rows).unwrap_or(usize::MAX), n_sub),
                        2,
                    )),
                );
                let locs = [
                    (1u64, 2u64),
                    (1, to_u64(add(self.meta.last_fl, 1))),
                    (1, to_u64(add(n_sub, 1))),
                    (to_u64(add(last_idx, 1)), to_u64(sub(sc_last, 1))),
                    (1, to_u64(add(self.meta.first_fl, 1))),
                ];
                for (k, (pg, v)) in locs.into_iter().enumerate() {
                    rs.u64(add(512, mul(16, k)), pg);
                    rs.u64(add(520, mul(16, k)), v);
                }
                let start0 = ceil8(add(PAGE_HEADER, mul(PTR, add(n_sub, 1))));
                let avail = sub(sub(self.page_len, self.meta.total), start0);
                rs.u64(120, to_u64(cap_for(avail, self.row_len)));
                // page 0 avec RowSize patché
                let page0 = c.page0.take().unwrap_or_default();
                let mut meta = self.meta.clone();
                if let Some(s) = meta.subs.first_mut() {
                    s.data = rs.0.clone();
                }
                fixed.push((0, build_cpage(&meta, &self.opts, self.page_len, &page0, 0)));
                // page marqueur
                let marker_idx = add(first, 1);
                let a = self.page_len.min(32_767);
                let n_tr = c.stats.len();
                let mut mk = Buf::zeros(self.page_len);
                mk.u32(
                    0,
                    self.opts.file_code ^ u32::try_from(add(marker_idx, 1)).unwrap_or(0),
                );
                mk.u64(24, 1);
                mk.u16(32, 0x9000);
                mk.u16(34, to_u16(add(n_tr, 1)));
                mk.u16(36, to_u16(n_tr));
                mk.u16(38, to_u16(sub(sub(a, 48), mul(18, n_tr))));
                let area = sub(a, mul(18, n_tr));
                let mut cum = 0u64;
                let mut recs: Vec<(u64, u64, u16)> = Vec::with_capacity(n_tr);
                for s in &c.stats {
                    cum = cum.saturating_add(to_u64(s.rows));
                    let pno = add(file_index(s.logical, c.first), 1);
                    recs.push((cum, to_u64(pno), to_u16(sub(s.blocks, 1))));
                }
                for (j, (cum_rows, pno, last)) in recs.iter().rev().enumerate() {
                    let o = add(area, mul(18, j));
                    mk.put(o, &cum_rows.to_be_bytes());
                    mk.u64(add(o, 8), *pno);
                    mk.u16(add(o, 16), *last);
                }
                fixed.push((to_u64(marker_idx), mk.0));
                to_u64(add(logical_pages, 1))
            }
        };
        // en-tête de fichier
        let header = self.header(page_count);
        self.report.rows = n_rows;
        Ok(Finished {
            header,
            fixed_pages: fixed,
            tail_pages: std::mem::take(&mut self.pending),
            page_count,
            page_length: self.opts.page_length,
            report: self.report,
        })
    }

    fn header(&self, page_count: u64) -> Vec<u8> {
        let o = &self.opts;
        let mut h = Buf::zeros(self.page_len);
        h.put(0, &MAGIC);
        h.put(32, &[0x33, 0x22, 0x00, 0x33, 0x33, 0x01, 0x02, 0x31]);
        h.put(40, &[0x01]);
        h.put(47, &[0x14]);
        h.put(50, &[0x03, 0x01, 0x18, 0x1F, 0x10, 0x11]);
        h.put(56, &[0x33, 0x22, 0x00, 0x33, 0x33, 0x01, 0x02, 0x31]);
        h.put(64, &[0x01]);
        h.put(65, &[0x33, 0x01, 0x23, 0x33]);
        h.put(70, &[o.encoding.code()]);
        h.put(71, &[0x14]);
        h.put(73, &[0x20]);
        h.put(74, &[0x03, 0x01]);
        h.put(84, b"SAS FILE");
        h.put(92, &[b' '; 32]);
        let name = o.table_name.as_bytes();
        h.put(92, name.get(..name.len().min(32)).unwrap_or(&[]));
        h.put(156, b"DATA    ");
        let created = o.created.to_le_bytes();
        h.put(168, &created);
        h.put(176, &created);
        h.u32(200, o.page_length);
        h.u32(204, o.page_length);
        h.u64(208, page_count);
        let mut put_padded = |off: usize, text: &str, width: usize| {
            let b = text.as_bytes();
            h.put(off, b.get(..b.len().min(width)).unwrap_or(&[]));
        };
        put_padded(224, &o.release, 8);
        put_padded(232, &o.host, 8);
        put_padded(248, &o.os_version, 16);
        put_padded(280, &o.os_machine, 8);
        let low = [created[0], created[1], created[2], created[3]];
        h.put(296, &low);
        let key = *b"LNoe";
        let mut x = [0u8; 4];
        for (i, b) in x.iter_mut().enumerate() {
            *b = low.get(i).copied().unwrap_or(0) ^ key.get(i).copied().unwrap_or(0);
        }
        for k in 0..3usize {
            h.put(add(300, mul(4, k)), &x);
        }
        h.u32(328, o.file_code);
        h.put(336, &created);
        h.0
    }
}

fn too_big(schema: &WriteSchema, opts: &WriteOptions, row_len: usize, need: usize) -> SchemaError {
    if row_len.saturating_add(48) > usize::try_from(opts.page_length).unwrap_or(usize::MAX)
        && schema.columns.len() < 64
    {
        return SchemaError::RowTooLong {
            row_length: row_len,
            page_length: opts.page_length,
        };
    }
    let n = schema.columns.len();
    let per = need.checked_div(n).unwrap_or(1).max(1);
    let max_for_page = usize::try_from(opts.page_length)
        .unwrap_or(0)
        .checked_div(per)
        .unwrap_or(0);
    SchemaError::TooManyColumns {
        columns: n,
        page_length: opts.page_length,
        max_for_page,
    }
}

/// Rang de fichier (0-based) de la page logique `j` : la page marqueur suit la première page portant des lignes.
fn file_index(logical: usize, first: Option<usize>) -> usize {
    match first {
        Some(f) if logical > f => add(logical, 1),
        _ => logical,
    }
}

fn push_ptr(out: &mut Vec<u8>, off: usize, len: usize, comp: u8, ptype: u8) {
    out.extend_from_slice(&to_u64(off).to_le_bytes());
    out.extend_from_slice(&to_u64(len).to_le_bytes());
    out.push(comp);
    out.push(ptype);
    out.extend_from_slice(&[0u8; 6]);
}

fn plain_data_page(
    opts: &WriteOptions,
    page_len: usize,
    row_len: usize,
    rmax: usize,
    rank: usize,
    rows: &[u8],
    n: usize,
) -> Vec<u8> {
    let mut pg = Buf::zeros(page_len);
    pg.u32(0, opts.file_code ^ u32::try_from(add(rank, 1)).unwrap_or(0));
    let free = sub(
        sub(sub(page_len, PAGE_HEADER), mul(n, row_len)),
        add(rmax, 7) / 8,
    );
    pg.u64(24, to_u64(free));
    pg.u16(32, 0x0100);
    pg.u16(34, to_u16(n));
    pg.put(PAGE_HEADER, rows);
    pg.0
}

/// Page d'une table compressée (`20` §4.2 point 6) : sous-en-têtes (page 0), lignes, pointeur terminal.
fn build_cpage(
    meta: &Meta,
    opts: &WriteOptions,
    page_len: usize,
    page: &CPage,
    idx: usize,
) -> Vec<u8> {
    let mut pg = Buf::zeros(page_len);
    pg.u32(0, opts.file_code ^ u32::try_from(add(idx, 1)).unwrap_or(0));
    let mut ptrs: Vec<(usize, usize, u8, u8)> = Vec::new();
    let mut off_end = page_len;
    if page.meta {
        for s in &meta.subs {
            off_end = sub(off_end, s.data.len());
            pg.put(off_end, &s.data);
            ptrs.push((off_end, s.data.len(), 0, s.ptype));
        }
    }
    for ((flag, len), data) in page.ptrs.iter().zip(&page.data) {
        off_end = sub(off_end, *len);
        pg.put(off_end, data);
        ptrs.push((off_end, *len, *flag, 1));
    }
    // valeur écrite par l'écrivain de référence validé (`ref_writer.py`) : les champs concernés sont libres (`20` §5.3)
    let end = add(add(32, mul(ptrs.len(), PTR)), PTR);
    ptrs.push((end, sub(off_end, end), 1, 1));
    for (i, (o, ln, comp, ty)) in ptrs.iter().enumerate() {
        let mut p = Vec::with_capacity(PTR);
        push_ptr(&mut p, *o, *ln, *comp, *ty);
        pg.put(add(PAGE_HEADER, mul(i, PTR)), &p);
    }
    pg.u64(24, to_u64(sub(off_end, end)));
    pg.u16(32, 0);
    pg.u16(34, to_u16(ptrs.len()));
    pg.u16(36, to_u16(ptrs.len()));
    pg.0
}

//! `SasFile` : ouverture (en-tête + pages de métadonnées seulement) et lecture des lignes.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use uniformat_sas::{
    HeaderParser, Limits, MetaScanner, PageDecoder, PageHeader, PageKind, ReadReport, RowRef,
    ScanProgress, Strictness, TableMeta, MIN_HEADER_BYTES,
};

use crate::error::Result;
use crate::plan::{PageInfo, PagePlan, PlanMode};
use uniformat_core::ByteSource;

/// Options d'ouverture.
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    /// Bornes sur les tailles lues dans le fichier.
    pub limits: Limits,
    /// Gestion des cas non vérifiés (ADR 0006).
    pub strictness: Strictness,
    /// Encodage imposé pour tout le texte (U22 : le code de l'en-tête n'est pas toujours fiable).
    pub encoding_override: Option<&'static uniformat_sas::Encoding>,
}

/// Table `.sas7bdat` ouverte sur une source d'octets.
#[derive(Debug)]
pub struct SasFile<S: ByteSource> {
    source: S,
    meta: Arc<TableMeta>,
    report: ReadReport,
    options: OpenOptions,
    /// Pages lues à l'ouverture (`0..scanned.len()`), la dernière étant la première page de données.
    scanned: Vec<PageInfo>,
}

impl<S: ByteSource> SasFile<S> {
    /// Ouvre la table : lit l'en-tête puis les pages jusqu'à la fin des métadonnées, jamais les pages de données.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie, ou erreur de format (magic, tailles hors bornes, fichier plus court que
    /// `header_length + page_count × page_length`, métadonnées incohérentes).
    pub fn open(source: S, options: OpenOptions) -> Result<Self> {
        let len = source.len();
        let head_len =
            usize::try_from(len.min(MIN_HEADER_BYTES as u64)).unwrap_or(MIN_HEADER_BYTES);
        let head = source.read_range(0, head_len)?;
        let mut report = ReadReport::new(options.strictness);
        let header = HeaderParser::parse(&head, &options.limits, &mut report)?;
        drop(head);
        let page_len = u64::from(header.page_length);
        let needed = page_len
            .checked_mul(header.page_count)
            .and_then(|pages| pages.checked_add(u64::from(header.header_length)))
            .ok_or(uniformat_sas::Error::Truncated {
                needed: u64::MAX,
                got: len,
            })?;
        if len < needed {
            return Err(uniformat_sas::Error::Truncated { needed, got: len }.into());
        }
        let mut scanner = MetaScanner::new(&header, &options.limits, options.strictness);
        if let Some(enc) = options.encoding_override {
            scanner = scanner.with_encoding(enc);
        }
        let page_bytes = usize::try_from(header.page_length).unwrap_or(0);
        let head_len = PageHeader::header_len(&header).min(page_bytes);
        let mut scanned = Vec::new();
        for k in 0..header.page_count {
            let offset = u64::from(header.header_length).saturating_add(k.saturating_mul(page_len));
            // L'en-tête de page (40 octets) dit si la page porte des métadonnées : on ne lit que le reste des pages
            // de métadonnées, jamais une page de données (critère de fin de J4 : schéma sans données).
            let head = source.read_range(offset, head_len)?;
            let ph = PageHeader::parse_prefix(&header, k, &head, &mut report)?;
            scanned.push(PageInfo {
                kind: ph.kind,
                physical_rows: ph.physical_rows().unwrap_or(0),
                has_deleted: ph.has_deleted(),
            });
            match ph.kind {
                PageKind::Data | PageKind::Compressed => break,
                PageKind::Other => {}
                _ => {
                    let rest = source.read_range(
                        offset.saturating_add(head_len as u64),
                        page_bytes.saturating_sub(head_len),
                    )?;
                    let mut page = Vec::with_capacity(page_bytes);
                    page.extend_from_slice(&head);
                    page.extend_from_slice(&rest);
                    if scanner.feed_page(k, &page)? == ScanProgress::Done {
                        break;
                    }
                }
            }
        }
        let (meta, scan_report) = scanner.finish()?;
        report.extend(&scan_report);
        Ok(Self {
            source,
            meta: Arc::new(meta),
            report,
            options,
            scanned,
        })
    }

    /// Options d'ouverture.
    pub fn options(&self) -> &OpenOptions {
        &self.options
    }

    /// Source d'octets.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Pages lues à l'ouverture (`0..n`), la dernière étant la première page de données si elle existe.
    pub fn scanned_pages(&self) -> &[PageInfo] {
        &self.scanned
    }

    /// Décodeur de lignes pour cette table, avec l'encodage imposé s'il y en a un.
    pub fn decoder(&self) -> PageDecoder<'_> {
        let d = PageDecoder::new(&self.meta, self.options.strictness);
        match self.options.encoding_override {
            Some(enc) => d.with_encoding(enc),
            None => d,
        }
    }

    /// Plan de pages (ADR 0002). Le mode rapide ne lit rien ; le mode complet lit l'en-tête de chaque page
    /// restante (40 octets en 64 bits). Une table compressée a toujours un plan séquentiel.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie ou de format.
    pub fn page_plan(&self, mode: PlanMode) -> Result<PagePlan> {
        if mode == PlanMode::Fast || self.meta.compression != uniformat_sas::Compression::None {
            return Ok(PagePlan::dense(&self.meta, &self.scanned));
        }
        let h = &self.meta.header;
        let mut infos = self.scanned.clone();
        let mut scratch = ReadReport::new(Strictness::Lenient);
        let head_len = PageHeader::header_len(h);
        for k in (infos.len() as u64)..h.page_count {
            let off = u64::from(h.header_length)
                .saturating_add(k.saturating_mul(u64::from(h.page_length)));
            let bytes = self.source.read_range(off, head_len)?;
            infos.push(PageInfo::from_header(&self.meta, k, &bytes, &mut scratch)?);
        }
        Ok(PagePlan::from_infos(&self.meta, &infos))
    }

    /// Octets des pages `pages`, en une seule lecture.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie, ou plage hors du fichier.
    pub fn read_pages(&self, pages: &Range<u64>) -> Result<Cow<'_, [u8]>> {
        let h = &self.meta.header;
        let plen = u64::from(h.page_length);
        let start = u64::from(h.header_length).saturating_add(pages.start.saturating_mul(plen));
        let len = pages.end.saturating_sub(pages.start).saturating_mul(plen);
        let len = usize::try_from(len).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "plage de pages trop grande",
            )
        })?;
        Ok(self.source.read_range(start, len)?)
    }

    /// Métadonnées complètes, sans lecture des données.
    pub fn meta(&self) -> &TableMeta {
        &self.meta
    }

    /// Métadonnées partageables entre fils (lecteurs de lots, partitions).
    pub fn meta_arc(&self) -> Arc<TableMeta> {
        Arc::clone(&self.meta)
    }

    /// Avertissements de l'ouverture.
    pub fn report(&self) -> &ReadReport {
        &self.report
    }

    /// Appelle `f` pour chaque ligne visible, dans l'ordre du fichier. Rend les avertissements du décodage.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie ou de format ; un flot compressé invalide rend `CorruptCompressed`.
    pub fn for_each_row<F: FnMut(RowRef<'_>)>(&self, mut f: F) -> Result<ReadReport> {
        let h = &self.meta.header;
        let page_len = u64::from(h.page_length);
        let page_bytes = usize::try_from(h.page_length).unwrap_or(0);
        let mut decoder = PageDecoder::new(&self.meta, self.options.strictness);
        if let Some(enc) = self.options.encoding_override {
            decoder = decoder.with_encoding(enc);
        }
        for k in 0..h.page_count {
            let offset = u64::from(h.header_length).saturating_add(k.saturating_mul(page_len));
            let page = self.source.read_range(offset, page_bytes)?;
            decoder.visit_rows(k, &page, &mut f)?;
        }
        Ok(decoder.report().clone())
    }
}

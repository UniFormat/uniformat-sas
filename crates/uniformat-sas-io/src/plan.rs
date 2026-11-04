//! Plan de pages (ADR 0002) : où sont les lignes, sans lire les pages de données.
//!
//! Le plan répond à deux besoins : découper un fichier en groupes de pages contigus (unité de lecture, de
//! parallélisme et de partage), et savoir combien de lignes physiques précèdent chaque groupe (indice de ligne,
//! plafond `row_count`). Deux constructions :
//!
//! - **complète** ([`PlanMode::Full`]) : l'en-tête de chaque page (40 octets) est lu ; le plan est exact ;
//! - **rapide** ([`PlanMode::Fast`]) : après les pages de métadonnées, les pages de données sont supposées pleines
//!   (`rmax` lignes, mesuré sur la première page de données) ; l'hypothèse est **vérifiée à la lecture** de chaque
//!   page ([`PagePlan::check_page`]) et une page qui la contredit rend [`IoError::PlanMismatch`].
//!
//! Une table compressée n'a pas de formule (`15` §2.1) : son plan est séquentiel (un seul décodeur, pages dans
//! l'ordre).

use std::ops::Range;

use uniformat_sas::{PageHeader, PageKind, ReadReport, TableMeta};

use crate::error::{IoError, Result};

/// Mode de construction du plan de pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanMode {
    /// Pages de données supposées pleines, vérifiées à la lecture (aucune lecture de page à la planification).
    #[default]
    Fast,
    /// En-tête de chaque page lu : plan exact, `page_count` petites lectures.
    Full,
}

/// Ce que le plan sait d'une page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInfo {
    /// Nature de la page.
    pub kind: PageKind,
    /// Lignes physiques de la page (suppressions comprises).
    pub physical_rows: u32,
    /// Le bitmap de suppression est significatif.
    pub has_deleted: bool,
}

impl PageInfo {
    /// Décode l'en-tête de page `index` (au moins [`PageHeader::header_len`] octets).
    ///
    /// # Errors
    /// Erreur du cœur si l'en-tête est invalide.
    pub fn from_header(
        meta: &TableMeta,
        index: u64,
        bytes: &[u8],
        report: &mut ReadReport,
    ) -> Result<Self> {
        let ph = PageHeader::parse_prefix(&meta.header, index, bytes, report)?;
        Ok(Self {
            kind: ph.kind,
            physical_rows: ph.physical_rows()?,
            has_deleted: ph.has_deleted(),
        })
    }
}

/// Groupe de pages contiguës : unité de lecture et de parallélisme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageGroup {
    /// Pages du groupe (numéros de page, base 0).
    pub pages: Range<u64>,
    /// Lignes physiques des pages précédentes.
    pub rows_before: u64,
    /// Lignes physiques du groupe.
    pub rows: u64,
}

#[derive(Debug, Clone)]
enum Rows {
    /// Lignes physiques cumulées avant chaque page (`page_count + 1` valeurs).
    Exact(Vec<u64>),
    /// Pages `< first_data` exactes, ensuite `rmax` lignes par page jusqu'à `row_count`.
    Dense {
        head: Vec<u64>,
        first_data: u64,
        rmax: u64,
        row_count: u64,
    },
    /// Table compressée : seul le total est connu.
    Sequential,
}

/// Plan de pages d'une table.
#[derive(Debug, Clone)]
pub struct PagePlan {
    page_count: u64,
    header_length: u64,
    page_length: u64,
    rows: Rows,
}

impl PagePlan {
    /// Plan exact d'après l'en-tête de **toutes** les pages.
    #[must_use]
    pub fn from_infos(meta: &TableMeta, infos: &[PageInfo]) -> Self {
        let mut cum = Vec::with_capacity(infos.len().saturating_add(1));
        let mut total = 0u64;
        cum.push(0);
        for i in infos {
            total = total.saturating_add(u64::from(i.physical_rows));
            cum.push(total);
        }
        Self::with_rows(meta, Rows::Exact(cum))
    }

    /// Plan rapide : `scanned` décrit les pages `0..scanned.len()` lues à l'ouverture (la dernière est la première
    /// page de données). Si elle ne l'est pas, ou si la table est compressée, le plan se replie sur le cas sûr.
    #[must_use]
    pub fn dense(meta: &TableMeta, scanned: &[PageInfo]) -> Self {
        if meta.compression != uniformat_sas::Compression::None {
            return Self::with_rows(meta, Rows::Sequential);
        }
        let scanned_len = scanned.len() as u64;
        if scanned_len >= meta.header.page_count {
            return Self::from_infos(meta, scanned);
        }
        let Some((last, head_infos)) = scanned.split_last() else {
            return Self::with_rows(meta, Rows::Sequential);
        };
        if last.kind != PageKind::Data || last.physical_rows == 0 {
            // Pas de page de données pour mesurer `rmax` : on ne devine rien.
            return Self::with_rows(meta, Rows::Sequential);
        }
        let mut head = Vec::with_capacity(scanned.len());
        let mut total = 0u64;
        head.push(0);
        for i in head_infos {
            total = total.saturating_add(u64::from(i.physical_rows));
            head.push(total);
        }
        let first_data = head_infos.len() as u64;
        Self::with_rows(
            meta,
            Rows::Dense {
                head,
                first_data,
                rmax: u64::from(last.physical_rows),
                row_count: meta.row_count,
            },
        )
    }

    fn with_rows(meta: &TableMeta, rows: Rows) -> Self {
        Self {
            page_count: meta.header.page_count,
            header_length: u64::from(meta.header.header_length),
            page_length: u64::from(meta.header.page_length),
            rows,
        }
    }

    /// Nombre de pages après l'en-tête.
    #[must_use]
    pub fn page_count(&self) -> u64 {
        self.page_count
    }

    /// Les groupes peuvent se lire indépendamment (table non compressée).
    #[must_use]
    pub fn is_splittable(&self) -> bool {
        !matches!(self.rows, Rows::Sequential)
    }

    /// Le plan vient de l'en-tête de toutes les pages (aucune hypothèse à vérifier).
    #[must_use]
    pub fn is_exact(&self) -> bool {
        matches!(self.rows, Rows::Exact(_))
    }

    /// Plage d'octets des pages `pages` dans le fichier.
    #[must_use]
    pub fn byte_range(&self, pages: &Range<u64>) -> Range<u64> {
        let at = |k: u64| {
            self.header_length
                .saturating_add(k.saturating_mul(self.page_length))
        };
        at(pages.start)..at(pages.end)
    }

    /// Lignes physiques avant la page `page` (`None` si le plan est séquentiel).
    #[must_use]
    pub fn rows_before(&self, page: u64) -> Option<u64> {
        match &self.rows {
            Rows::Exact(cum) => cum
                .get(usize::try_from(page).ok()?)
                .or_else(|| cum.last())
                .copied(),
            Rows::Dense {
                head,
                first_data,
                rmax,
                row_count,
            } => {
                if page <= *first_data {
                    head.get(usize::try_from(page).ok()?).copied()
                } else {
                    let base = head.last().copied().unwrap_or(0);
                    let dense = page
                        .saturating_sub(*first_data)
                        .saturating_mul(*rmax)
                        .saturating_add(base);
                    Some(dense.min((*row_count).max(base)))
                }
            }
            Rows::Sequential => None,
        }
    }

    /// Lignes physiques attendues sur la page `page`.
    fn expected_rows(&self, page: u64) -> Option<u64> {
        let before = self.rows_before(page)?;
        match &self.rows {
            Rows::Exact(cum) => cum
                .get(usize::try_from(page.saturating_add(1)).ok()?)
                .map(|n| n.saturating_sub(before)),
            Rows::Dense {
                head,
                first_data,
                rmax,
                row_count,
            } => {
                if page < *first_data {
                    head.get(usize::try_from(page.saturating_add(1)).ok()?)
                        .map(|n| n.saturating_sub(before))
                } else {
                    Some((*rmax).min(row_count.saturating_sub(before)))
                }
            }
            Rows::Sequential => None,
        }
    }

    /// Vérifie qu'une page lue (`found` lignes physiques) est conforme au plan.
    ///
    /// # Errors
    /// [`IoError::PlanMismatch`] si le nombre de lignes diffère de celui que le plan suppose.
    pub fn check_page(&self, page: u64, found: u32) -> Result<()> {
        match self.expected_rows(page) {
            Some(e) if e != u64::from(found) => Err(IoError::PlanMismatch {
                page,
                expected: e,
                found: u64::from(found),
            }),
            _ => Ok(()),
        }
    }

    /// Découpe en `parts` groupes contigus au plus, équilibrés en lignes physiques (au moins une page chacun).
    /// Un plan séquentiel rend un seul groupe.
    #[must_use]
    pub fn split(&self, parts: usize) -> Vec<PageGroup> {
        let parts = u64::try_from(parts.max(1)).unwrap_or(1);
        if !self.is_splittable() || self.page_count == 0 {
            return self.single();
        }
        let total = self.rows_before(self.page_count).unwrap_or(0);
        let mut groups = Vec::new();
        let mut start = 0u64;
        for part in 1..=parts {
            if start >= self.page_count {
                break;
            }
            let end = if part == parts {
                self.page_count
            } else {
                // première page dont le début atteint la fraction `part / parts` des lignes
                let target = total
                    .checked_mul(part)
                    .and_then(|v| v.checked_div(parts))
                    .unwrap_or(0);
                self.first_page_at_or_after(target, start.saturating_add(1))
            };
            if end > start {
                groups.push(self.group(start..end));
                start = end;
            }
        }
        groups
    }

    /// Découpe en groupes d'environ `bytes` octets (au moins une page). Un plan séquentiel rend un seul groupe.
    #[must_use]
    pub fn chunk(&self, bytes: u64) -> Vec<PageGroup> {
        if !self.is_splittable() || self.page_count == 0 {
            return self.single();
        }
        let per = (bytes / self.page_length.max(1)).max(1);
        let mut groups = Vec::new();
        let mut start = 0u64;
        while start < self.page_count {
            let end = start.saturating_add(per).min(self.page_count);
            groups.push(self.group(start..end));
            start = end;
        }
        groups
    }

    fn single(&self) -> Vec<PageGroup> {
        if self.page_count == 0 {
            return Vec::new();
        }
        vec![PageGroup {
            pages: 0..self.page_count,
            rows_before: 0,
            rows: self.rows_before(self.page_count).unwrap_or(0),
        }]
    }

    fn group(&self, pages: Range<u64>) -> PageGroup {
        let before = self.rows_before(pages.start).unwrap_or(0);
        let after = self.rows_before(pages.end).unwrap_or(before);
        PageGroup {
            pages,
            rows_before: before,
            rows: after.saturating_sub(before),
        }
    }

    /// Plus petite page `p ≥ min` telle que `rows_before(p) ≥ target` (au plus `page_count`).
    fn first_page_at_or_after(&self, target: u64, min: u64) -> u64 {
        let (mut lo, mut hi) = (min.min(self.page_count), self.page_count);
        while lo < hi {
            let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
            if self.rows_before(mid).unwrap_or(u64::MAX) >= target {
                hi = mid;
            } else {
                lo = mid.saturating_add(1);
            }
        }
        lo
    }
}

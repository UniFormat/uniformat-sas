//! `BatchReader` : lecture synchrone d'une plage de pages en `RecordBatch`.

use std::ops::Range;
use std::sync::Arc;

use arrow_array::{RecordBatch, RecordBatchReader};
use arrow_schema::{ArrowError, SchemaRef};
use uniformat_sas::ReadReport;
use uniformat_sas_io::{ByteSource, PageGroup, PagePlan, SasFile};

use crate::assembler::{BatchAssembler, ReadStats};
use crate::error::{Error, Result};
use crate::options::ReadOptions;

/// Lecteur de lots Arrow sur une plage de pages (ADR 0002, unité de parallélisme et de partage).
///
/// Les lots sortent dans l'ordre des pages. Une table compressée se lit depuis la première page.
pub struct BatchReader<S: ByteSource> {
    file: Arc<SasFile<S>>,
    assembler: BatchAssembler,
    plan: Option<Arc<PagePlan>>,
    groups: std::vec::IntoIter<Range<u64>>,
    buf: Vec<u8>,
    buf_first_page: u64,
    buf_pos: usize,
    page_len: usize,
    finished: bool,
    failed: bool,
}

impl<S: ByteSource> std::fmt::Debug for BatchReader<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchReader")
            .field("assembler", &self.assembler)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

fn chunks(pages: &Range<u64>, per: u64) -> Vec<Range<u64>> {
    let per = per.max(1);
    let mut out = Vec::new();
    let mut start = pages.start;
    while start < pages.end {
        let end = start.saturating_add(per).min(pages.end);
        out.push(start..end);
        start = end;
    }
    out
}

impl<S: ByteSource> BatchReader<S> {
    /// Lecteur pour les pages `pages` (`projection` : indices du schéma complet ; `limit` : lignes visibles au plus).
    ///
    /// Une plage qui ne commence pas à la page 0 demande le plan de pages (`opts.plan`), et n'existe que pour une
    /// table non compressée.
    ///
    /// # Errors
    /// Projection invalide, plage hors du fichier, table compressée lue à partir d'une autre page que la première,
    /// ou erreur d'entrée/sortie du plan.
    pub fn new(
        file: Arc<SasFile<S>>,
        pages: Range<u64>,
        projection: Option<Vec<usize>>,
        limit: Option<u64>,
        opts: ReadOptions,
    ) -> Result<Self> {
        if pages.end > file.meta().header.page_count || pages.start > pages.end {
            return Err(Error::Unsupported("plage de pages hors du fichier"));
        }
        let (rows_before, plan) = if pages.start == 0 {
            (0, None)
        } else {
            let plan = Arc::new(file.page_plan(opts.plan)?);
            let rows_before = plan.rows_before(pages.start).ok_or(Error::Unsupported(
                "table compressée : lecture depuis la première page seulement",
            ))?;
            (rows_before, Some(plan))
        };
        Self::build(file, pages, plan, rows_before, projection, limit, opts)
    }

    /// Lecteur d'un groupe du plan (partition) : le nombre de lignes de chaque page est vérifié contre `plan`.
    ///
    /// # Errors
    /// Projection invalide.
    pub fn with_group(
        file: Arc<SasFile<S>>,
        group: &PageGroup,
        plan: Arc<PagePlan>,
        projection: Option<Vec<usize>>,
        limit: Option<u64>,
        opts: ReadOptions,
    ) -> Result<Self> {
        let rows_before = if plan.is_splittable() {
            group.rows_before
        } else {
            0
        };
        let plan = plan.is_splittable().then_some(plan);
        Self::build(
            file,
            group.pages.clone(),
            plan,
            rows_before,
            projection,
            limit,
            opts,
        )
    }

    fn build(
        file: Arc<SasFile<S>>,
        pages: Range<u64>,
        plan: Option<Arc<PagePlan>>,
        rows_before: u64,
        projection: Option<Vec<usize>>,
        limit: Option<u64>,
        opts: ReadOptions,
    ) -> Result<Self> {
        let assembler = BatchAssembler::new(
            file.meta_arc(),
            &opts,
            projection.as_deref(),
            limit,
            rows_before,
        )?;
        let page_len = usize::try_from(file.meta().header.page_length).unwrap_or(usize::MAX);
        let per = (opts.group_bytes / u64::from(file.meta().header.page_length).max(1)).max(1);
        Ok(Self {
            groups: chunks(&pages, per).into_iter(),
            file,
            assembler,
            plan,
            buf: Vec::new(),
            buf_first_page: 0,
            buf_pos: 0,
            page_len,
            finished: false,
            failed: false,
        })
    }

    /// Schéma des lots (projection appliquée).
    #[must_use]
    pub fn schema(&self) -> SchemaRef {
        self.assembler.schema()
    }

    /// Écarts comptés jusqu'ici.
    #[must_use]
    pub fn stats(&self) -> &ReadStats {
        self.assembler.stats()
    }

    /// Avertissements du décodage.
    #[must_use]
    pub fn report(&self) -> Option<&ReadReport> {
        self.assembler.report()
    }

    /// Adaptateur `RecordBatchReader` d'Arrow (les erreurs deviennent `ArrowError::ExternalError`).
    #[must_use]
    pub fn into_arrow(self) -> ArrowBatchReader<S> {
        ArrowBatchReader(self)
    }

    fn advance(&mut self) -> Result<Option<RecordBatch>> {
        loop {
            if self.assembler.is_full() || (self.finished && self.assembler.pending_rows() > 0) {
                return self.assembler.take_batch();
            }
            if self.finished {
                return Ok(None);
            }
            if self.assembler.limit_reached() {
                self.finished = true;
                continue;
            }
            if self.buf_pos >= self.buf.len() {
                let Some(group) = self.groups.next() else {
                    self.finished = true;
                    continue;
                };
                self.buf = self.file.read_pages(&group)?.into_owned();
                self.buf_first_page = group.start;
                self.buf_pos = 0;
            }
            let end = self.buf_pos.saturating_add(self.page_len);
            let page_index = self
                .buf_first_page
                .saturating_add((self.buf_pos / self.page_len.max(1)) as u64);
            let Some(page) = self.buf.get(self.buf_pos..end) else {
                return Err(Error::Format(uniformat_sas::Error::Truncated {
                    needed: end as u64,
                    got: self.buf.len() as u64,
                }));
            };
            self.assembler
                .push_page(page_index, page, self.plan.as_deref())?;
            self.buf_pos = end;
        }
    }
}

impl<S: ByteSource> Iterator for BatchReader<S> {
    type Item = Result<RecordBatch>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        match self.advance() {
            Ok(Some(b)) => Some(Ok(b)),
            Ok(None) => None,
            Err(e) => {
                self.failed = true;
                self.finished = true;
                self.buf = Vec::new();
                self.buf_pos = 0;
                // la partie déjà décodée du lot en cours est perdue : l'erreur termine le flux
                Some(Err(e))
            }
        }
    }
}

/// [`BatchReader`] vu comme `RecordBatchReader` d'Arrow.
#[derive(Debug)]
pub struct ArrowBatchReader<S: ByteSource>(BatchReader<S>);

impl<S: ByteSource> Iterator for ArrowBatchReader<S> {
    type Item = std::result::Result<RecordBatch, ArrowError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|r| r.map_err(|e| ArrowError::ExternalError(Box::new(e))))
    }
}

impl<S: ByteSource> RecordBatchReader for ArrowBatchReader<S> {
    fn schema(&self) -> SchemaRef {
        self.0.schema()
    }
}

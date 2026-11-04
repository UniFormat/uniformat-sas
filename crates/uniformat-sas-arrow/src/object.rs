//! Adaptateur asynchrone `object_store` (ADR 0002) : fichiers locaux, S3, HTTP.
//!
//! Le cœur et `uniformat-sas-io` restent synchrones. Ici, les plages d'octets sont obtenues par `get_range` puis passées
//! au même [`BatchAssembler`] que le lecteur synchrone ; le décodage s'exécute sur le pool bloquant de tokio.
//!
//! Requêtes émises pour lire un fichier : une `head` (taille), une lecture du préfixe (en-tête et pages de
//! métadonnées, 1 Mio par défaut, agrandie si les métadonnées le demandent), puis **une requête par groupe de pages**
//! (≈ `group_bytes`). Soit au plus `2 + nombre de groupes` requêtes (plan rapide).

use std::borrow::Cow;
use std::collections::VecDeque;
use std::io;
use std::ops::Range;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use bytes::Bytes;
use futures::stream::{self, BoxStream, StreamExt};
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt};
use uniformat_sas::{PageHeader, ReadReport, Strictness, TableMeta};
use uniformat_sas_io::{ByteSource, IoError, PageGroup, PageInfo, PagePlan, PlanMode, SasFile};

use crate::assembler::BatchAssembler;
use crate::error::{Error, Result};
use crate::options::ReadOptions;
use crate::schema::arrow_schema;

/// Taille du premier préfixe lu : en-tête et pages de métadonnées d'une table ordinaire.
const PREFIX_BYTES: u64 = 1024 * 1024;
/// Pages dont l'en-tête est lu par requête pour le plan complet.
const PLAN_BATCH: usize = 256;
/// Groupes de pages récupérés en avance.
const PREFETCH: usize = 2;

/// Le préfixe lu ne couvre pas la plage demandée.
#[derive(Debug)]
struct BeyondPrefix(u64);

impl std::fmt::Display for BeyondPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "lecture au-delà du préfixe chargé (jusqu'à l'octet {})",
            self.0
        )
    }
}

impl std::error::Error for BeyondPrefix {}

/// Début d'un fichier en mémoire, taille réelle connue : ne sert que l'ouverture.
#[derive(Debug)]
struct PrefixSource {
    data: Bytes,
    size: u64,
}

impl ByteSource for PrefixSource {
    fn len(&self) -> u64 {
        self.size
    }

    fn read_range(&self, offset: u64, len: usize) -> io::Result<Cow<'_, [u8]>> {
        let end = offset.checked_add(len as u64).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "plage hors de la source")
        })?;
        if end > self.size {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "plage hors de la source",
            ));
        }
        let (s, e) = (usize::try_from(offset), usize::try_from(end));
        match (s, e) {
            (Ok(s), Ok(e)) if e <= self.data.len() => Ok(Cow::Borrowed(&self.data[s..e])),
            _ => Err(io::Error::other(BeyondPrefix(end))),
        }
    }
}

fn beyond_prefix(e: &IoError) -> Option<u64> {
    match e {
        IoError::Io(io) => io.get_ref()?.downcast_ref::<BeyondPrefix>().map(|b| b.0),
        _ => None,
    }
}

/// Table `.sas7bdat` dans un magasin d'objets, ouverte sur ses seules métadonnées.
#[derive(Debug, Clone)]
pub struct ObjectFile {
    store: Arc<dyn ObjectStore>,
    path: Path,
    file: Arc<SasFile<PrefixSource>>,
    opts: ReadOptions,
}

impl ObjectFile {
    /// Ouvre la table : une requête `head`, une lecture du préfixe (plus si les métadonnées le demandent).
    ///
    /// # Errors
    /// Erreur du magasin, ou de format.
    pub async fn open(store: Arc<dyn ObjectStore>, path: Path, opts: ReadOptions) -> Result<Self> {
        let size = store.head(&path).await?.size;
        let mut want = size.min(PREFIX_BYTES);
        let file = loop {
            let data = store.get_range(&path, 0..want).await?;
            let src = PrefixSource { data, size };
            match SasFile::open(src, opts.open_options()) {
                Ok(f) => break f,
                Err(e) => match beyond_prefix(&e) {
                    Some(needed) if want < size => {
                        want = size.min(needed.max(want.saturating_mul(2)));
                    }
                    _ => return Err(e.into()),
                },
            }
        };
        Ok(Self {
            store,
            path,
            file: Arc::new(file),
            opts,
        })
    }

    /// Métadonnées complètes.
    #[must_use]
    pub fn meta(&self) -> &TableMeta {
        self.file.meta()
    }

    /// Métadonnées partageables.
    #[must_use]
    pub fn meta_arc(&self) -> Arc<TableMeta> {
        self.file.meta_arc()
    }

    /// Avertissements de l'ouverture.
    #[must_use]
    pub fn report(&self) -> &ReadReport {
        self.file.report()
    }

    /// Options de lecture.
    #[must_use]
    pub fn options(&self) -> &ReadOptions {
        &self.opts
    }

    /// Taille de l'objet.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.file.source().len()
    }

    /// Schéma Arrow complet (aucune lecture de données).
    #[must_use]
    pub fn schema(&self) -> SchemaRef {
        arrow_schema(self.file.meta(), &self.opts)
    }

    /// Plan de pages selon `opts.plan`. Le plan rapide ne lit rien ; le plan complet lit les en-têtes des pages
    /// restantes par lots de 256 pages.
    ///
    /// # Errors
    /// Erreur du magasin ou de format.
    pub async fn page_plan(&self) -> Result<PagePlan> {
        let meta = self.file.meta();
        if self.opts.plan == PlanMode::Fast || meta.compression != uniformat_sas::Compression::None
        {
            return Ok(self.file.page_plan(PlanMode::Fast)?);
        }
        let h = &meta.header;
        let head_len = PageHeader::header_len(h) as u64;
        let mut infos: Vec<PageInfo> = self.file.scanned_pages().to_vec();
        let mut scratch = ReadReport::new(Strictness::Lenient);
        let start = infos.len() as u64;
        let pages: Vec<u64> = (start..h.page_count).collect();
        for chunk in pages.chunks(PLAN_BATCH) {
            let ranges: Vec<Range<u64>> = chunk
                .iter()
                .map(|&k| {
                    let off = u64::from(h.header_length) + k * u64::from(h.page_length);
                    off..off + head_len
                })
                .collect();
            let got = self.store.get_ranges(&self.path, &ranges).await?;
            for (&k, bytes) in chunk.iter().zip(&got) {
                infos.push(PageInfo::from_header(meta, k, bytes, &mut scratch)?);
            }
        }
        Ok(PagePlan::from_infos(meta, &infos))
    }

    /// Flux de lots pour un groupe de pages. Les groupes lus ont environ `group_bytes` octets (une requête chacun) ;
    /// `plan` (s'il est donné) vérifie le nombre de lignes de chaque page. Nécessite un runtime tokio.
    ///
    /// # Errors
    /// Projection invalide.
    pub fn stream(
        &self,
        group: &PageGroup,
        plan: Option<Arc<PagePlan>>,
        projection: Option<Vec<usize>>,
        limit: Option<u64>,
    ) -> Result<BoxStream<'static, Result<RecordBatch>>> {
        let h = &self.file.meta().header;
        let page_len = u64::from(h.page_length).max(1);
        let rows_before = match &plan {
            Some(p) if p.is_splittable() => group.rows_before,
            _ => 0,
        };
        let plan = plan.filter(|p| p.is_splittable());
        let assembler = BatchAssembler::new(
            self.file.meta_arc(),
            &self.opts,
            projection.as_deref(),
            limit,
            rows_before,
        )?;
        let per = (self.opts.group_bytes / page_len).max(1);
        let mut chunks = Vec::new();
        let mut start = group.pages.start;
        while start < group.pages.end {
            let end = start.saturating_add(per).min(group.pages.end);
            chunks.push(start..end);
            start = end;
        }
        let (store, path) = (Arc::clone(&self.store), self.path.clone());
        let (hl, plen) = (u64::from(h.header_length), page_len);
        let fetches = stream::iter(chunks)
            .map(move |pages| {
                let (store, path) = (Arc::clone(&store), path.clone());
                async move {
                    let range = hl + pages.start * plen..hl + pages.end * plen;
                    let bytes = store.get_range(&path, range).await?;
                    Ok::<_, Error>((pages.start, bytes))
                }
            })
            .buffered(PREFETCH)
            .boxed();
        struct State {
            fetches: BoxStream<'static, Result<(u64, Bytes)>>,
            assembler: Option<BatchAssembler>,
            plan: Option<Arc<PagePlan>>,
            ready: VecDeque<RecordBatch>,
            done: bool,
        }
        let state = State {
            fetches,
            assembler: Some(assembler),
            plan,
            ready: VecDeque::new(),
            done: false,
        };
        Ok(stream::unfold(state, |mut st| async move {
            loop {
                if let Some(b) = st.ready.pop_front() {
                    return Some((Ok(b), st));
                }
                if st.done {
                    return None;
                }
                let mut asm = st.assembler.take()?;
                if asm.limit_reached() {
                    st.done = true;
                    let last = asm.take_batch();
                    st.assembler = Some(asm);
                    match last {
                        Ok(Some(b)) => return Some((Ok(b), st)),
                        Ok(None) => return None,
                        Err(e) => return Some((Err(e), st)),
                    }
                }
                match st.fetches.next().await {
                    None => {
                        st.done = true;
                        let last = asm.take_batch();
                        st.assembler = Some(asm);
                        match last {
                            Ok(Some(b)) => return Some((Ok(b), st)),
                            Ok(None) => return None,
                            Err(e) => return Some((Err(e), st)),
                        }
                    }
                    Some(Err(e)) => {
                        st.done = true;
                        return Some((Err(e), st));
                    }
                    Some(Ok((first_page, bytes))) => {
                        let plan = st.plan.clone();
                        let joined = tokio::task::spawn_blocking(move || {
                            let r = decode_group(&mut asm, first_page, &bytes, plan.as_deref());
                            (asm, r)
                        })
                        .await;
                        match joined {
                            Ok((asm, Ok(batches))) => {
                                st.assembler = Some(asm);
                                st.ready.extend(batches);
                            }
                            Ok((_, Err(e))) => {
                                st.done = true;
                                return Some((Err(e), st));
                            }
                            Err(_) => {
                                st.done = true;
                                return Some((
                                    Err(Error::Unsupported("tâche de décodage interrompue")),
                                    st,
                                ));
                            }
                        }
                    }
                }
            }
        })
        .boxed())
    }

    /// Flux de tous les lots du fichier, dans l'ordre des pages (lecture séquentielle).
    ///
    /// # Errors
    /// Projection invalide.
    pub fn stream_all(
        &self,
        projection: Option<Vec<usize>>,
        limit: Option<u64>,
    ) -> Result<BoxStream<'static, Result<RecordBatch>>> {
        let pages = 0..self.file.meta().header.page_count;
        let group = PageGroup {
            pages,
            rows_before: 0,
            rows: 0,
        };
        self.stream(&group, None, projection, limit)
    }
}

/// Décode les pages d'un groupe et rend les lots complets ; le dernier lot partiel reste dans l'assembleur.
fn decode_group(
    asm: &mut BatchAssembler,
    first_page: u64,
    bytes: &[u8],
    plan: Option<&PagePlan>,
) -> Result<Vec<RecordBatch>> {
    let plen = usize::try_from(asm.page_length())
        .unwrap_or(usize::MAX)
        .max(1);
    let mut out = Vec::new();
    for (i, page) in bytes.chunks_exact(plen).enumerate() {
        if asm.limit_reached() {
            break;
        }
        asm.push_page(first_page.saturating_add(i as u64), page, plan)?;
        if asm.is_full() {
            if let Some(b) = asm.take_batch()? {
                out.push(b);
            }
        }
    }
    Ok(out)
}

//! Adaptateur asynchrone `object_store` (ADR 0002) : fichiers locaux, S3, HTTP.
//!
//! Le cœur et `uniformat-sas-io` restent synchrones. Ici, les plages d'octets sont obtenues par `get_range` puis passées
//! au même [`BatchAssembler`] que le lecteur synchrone ; le décodage s'exécute sur le pool bloquant de tokio.
//!
//! Requêtes émises pour lire un fichier : une `head` (taille), une lecture du préfixe (en-tête et pages de
//! métadonnées, 1 Mio par défaut, agrandie si les métadonnées le demandent), puis **une requête par groupe de pages**
//! (≈ `group_bytes`). Soit au plus `2 + nombre de groupes` requêtes (plan rapide).

use std::borrow::Cow;
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
        let prefix_limit = opts.max_metadata_bytes as u64;
        let mut want = size.min(PREFIX_BYTES).min(prefix_limit);
        let file = loop {
            let data = store.get_range(&path, 0..want).await?;
            let src = PrefixSource { data, size };
            match SasFile::open(src, opts.open_options()) {
                Ok(f) => break f,
                Err(e) => match beyond_prefix(&e) {
                    Some(needed) if want < size => {
                        if needed > prefix_limit {
                            return Err(Error::ResourceLimit(
                                "métadonnées distantes trop volumineuses",
                            ));
                        }
                        want = size
                            .min(prefix_limit)
                            .min(needed.max(want.saturating_mul(2)));
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
        // A remote store can advertise an enormous object without transferring its contents.
        // Bound the full-plan vectors before any allocation or per-page network requests.
        if h.page_count > (self.opts.max_metadata_bytes / 64) as u64 {
            return Err(Error::ResourceLimit(
                "plan de pages distant trop volumineux",
            ));
        }
        let head_len = PageHeader::header_len(h) as u64;
        let mut infos: Vec<PageInfo> = self.file.scanned_pages().to_vec();
        let mut scratch = ReadReport::new(Strictness::Lenient);
        let start = infos.len() as u64;
        let mut next_page = start;
        while next_page < h.page_count {
            let end = next_page
                .saturating_add(PLAN_BATCH as u64)
                .min(h.page_count);
            let chunk: Vec<u64> = (next_page..end).collect();
            next_page = end;
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
        if group.pages.start > group.pages.end || group.pages.end > h.page_count {
            return Err(Error::Unsupported("plage de pages distante invalide"));
        }
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
        // Keep only the current range: precomputing every range makes LIMIT ineffective
        // against an attacker-controlled remote object size.
        let mut start = group.pages.start;
        let last = group.pages.end;
        let chunks = std::iter::from_fn(move || {
            if start >= last {
                return None;
            }
            let end = start.saturating_add(per).min(last);
            let range = start..end;
            start = end;
            Some(range)
        });
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
            pending: Option<(u64, Bytes, usize)>,
            done: bool,
        }
        let state = State {
            fetches,
            assembler: Some(assembler),
            plan,
            pending: None,
            done: false,
        };
        Ok(stream::unfold(state, |mut st| async move {
            loop {
                if st.done {
                    return None;
                }
                let mut asm = st.assembler.take()?;
                if asm.limit_reached() {
                    st.done = true;
                    return asm.take_batch().transpose().map(|batch| (batch, st));
                }
                let (first_page, bytes, mut pos) = match st.pending.take() {
                    Some(pending) => pending,
                    None => match st.fetches.next().await {
                        None => {
                            st.done = true;
                            return asm.take_batch().transpose().map(|batch| (batch, st));
                        }
                        Some(Err(e)) => {
                            st.done = true;
                            return Some((Err(e), st));
                        }
                        Some(Ok((first, bytes))) => (first, bytes, 0),
                    },
                };
                let plan = st.plan.clone();
                // Decode at most one output batch. Accumulating all expanded batches of an
                // input group defeats a per-batch budget when projection/compression amplifies it.
                let joined = tokio::task::spawn_blocking(move || {
                    let batch =
                        decode_next_batch(&mut asm, first_page, &bytes, &mut pos, plan.as_deref());
                    (asm, bytes, pos, batch)
                })
                .await;
                match joined {
                    Ok((asm, bytes, pos, Ok(batch))) => {
                        st.assembler = Some(asm);
                        if pos < bytes.len() {
                            st.pending = Some((first_page, bytes, pos));
                        }
                        if let Some(batch) = batch {
                            return Some((Ok(batch), st));
                        }
                    }
                    Ok((_, _, _, Err(e))) => {
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

/// Decode only until the next complete batch; keep the remaining input group for the next poll.
fn decode_next_batch(
    asm: &mut BatchAssembler,
    first_page: u64,
    bytes: &[u8],
    pos: &mut usize,
    plan: Option<&PagePlan>,
) -> Result<Option<RecordBatch>> {
    let plen = usize::try_from(asm.page_length())
        .unwrap_or(usize::MAX)
        .max(1);
    if !bytes.len().is_multiple_of(plen) {
        return Err(Error::Unsupported("groupe de pages distant tronqué"));
    }
    while *pos < bytes.len() && !asm.limit_reached() {
        let index = first_page.saturating_add((*pos / plen) as u64);
        asm.push_page(index, &bytes[*pos..*pos + plen], plan)?;
        *pos += plen;
        if asm.is_full() {
            return asm.take_batch();
        }
    }
    Ok(None)
}

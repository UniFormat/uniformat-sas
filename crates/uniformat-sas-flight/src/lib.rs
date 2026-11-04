//! Service Arrow Flight sur un dossier de fichiers `.sas7bdat` (jalon J7, ADR 0002 et 0004).
//!
//! - `GetFlightInfo` rend le **schéma sans lire de page de données** (ouverture de l'en-tête et des pages de
//!   métadonnées seulement) et **un `FlightEndpoint` par groupe de pages** (plan de pages de l'ADR 0002) ;
//! - `DoGet(ticket)` lit un groupe de pages (fichier + plage + projection + `LIMIT`) et le sert en flux de lots ; les `N`
//!   endpoints se lisent en parallèle et redonnent les lignes du fichier ;
//! - `ListFlights` énumère les fichiers du dossier ; `GetSchema` rend le schéma seul ;
//! - authentification déléguée : jeton porteur optionnel (`authorization: Bearer …`), sinon à l'amont (proxy, mTLS).
//!
//! Les tickets reviennent d'un client : fichier, plage de pages et options sont **revalidés** à chaque `DoGet`
//! (chemin confiné au dossier racine, plage dans le fichier, `rows_before` recalculé par le plan).
#![forbid(unsafe_code)]

mod resource;
use resource::RequestBudget;
pub use resource::ServiceLimits;
use tokio::sync::Semaphore;
use tokio::time::{timeout_at, Instant};

use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_flight::encode::FlightDataEncoderBuilder;
use arrow_flight::error::FlightError;
use arrow_flight::flight_service_server::FlightService;
use arrow_flight::{
    Action, ActionType, Criteria, Empty, FlightData, FlightDescriptor, FlightEndpoint, FlightInfo,
    HandshakeRequest, HandshakeResponse, PollInfo, PutResult, SchemaResult, Ticket,
};
use arrow_schema::SchemaRef;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use prost::Message;
use serde::{Deserialize, Serialize};
use tonic::{Request, Response, Status, Streaming};
use uniformat_sas::{Compression, Encoding};
use uniformat_sas_arrow::{
    arrow_schema, BatchReader, DateMode, MissingMode, ReadOptions, TrailingBlanks,
};
use uniformat_sas_io::{CountingSource, FileSource, PageGroup, PlanMode, SasFile};

/// Nombre maximal de partitions demandées.
const MAX_PARTITIONS: usize = 1_024;

/// Options de lecture communes au descripteur et au ticket.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReadSpec {
    /// Colonnes (noms du schéma Arrow, colonnes annexes comprises) ; toutes si absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    /// Nombre maximal de lignes (un seul endpoint, exact).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    /// `null` (défaut) ou `sidecar` : colonnes annexes des manquants spéciaux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
    /// `native` (défaut) ou `float` : dates en valeur SAS brute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dates: Option<String>,
    /// Encodage imposé pour le texte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// `trim` (défaut) ou `keep`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing_blanks: Option<String>,
    /// `fast` (défaut) ou `full` : plan de pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Lignes visées par lot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_rows: Option<usize>,
}

/// Commande d'un descripteur `CMD` (JSON) : un fichier, des options et un nombre de partitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Command {
    /// Fichier, relatif au dossier racine.
    pub path: String,
    /// Nombre d'endpoints voulus (au plus ; une table compressée n'en a qu'un).
    #[serde(default)]
    pub partitions: Option<usize>,
    /// Options de lecture.
    #[serde(flatten)]
    pub spec: ReadSpec,
}

/// Contenu d'un ticket (JSON) : une plage de pages d'un fichier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBody {
    /// Fichier, relatif au dossier racine.
    pub path: String,
    /// Première page et page de fin (exclue).
    pub pages: (u64, u64),
    /// Options de lecture.
    #[serde(flatten)]
    pub spec: ReadSpec,
}

/// Compteurs d'activité (octets lus du fichier pour les schémas et pour les données).
#[derive(Debug, Default)]
pub struct Stats {
    /// Octets lus par `GetFlightInfo`, `GetSchema` et `ListFlights` (en-tête et métadonnées).
    pub info_bytes: AtomicU64,
    /// Octets lus par `DoGet`.
    pub data_bytes: AtomicU64,
    /// Appels `GetFlightInfo`.
    pub info_calls: AtomicU64,
    /// Appels `DoGet`.
    pub get_calls: AtomicU64,
}

/// Service Flight sur un dossier racine.
#[derive(Debug, Clone)]
pub struct SasFlightService {
    root: Arc<PathBuf>,
    stats: Arc<Stats>,
    default_partitions: usize,
    limits: ServiceLimits,
    slots: Arc<Semaphore>,
}

fn internal(e: impl std::fmt::Display) -> Status {
    Status::internal(e.to_string())
}

fn invalid(e: impl std::fmt::Display) -> Status {
    Status::invalid_argument(e.to_string())
}

fn map_error(e: uniformat_sas_arrow::Error) -> Status {
    match e {
        uniformat_sas_arrow::Error::Io(uniformat_sas_io::IoError::Io(io))
            if io.kind() == std::io::ErrorKind::NotFound =>
        {
            Status::not_found(io.to_string())
        }
        uniformat_sas_arrow::Error::Projection(_) => Status::invalid_argument(e.to_string()),
        uniformat_sas_arrow::Error::ResourceLimit(_) => Status::resource_exhausted(e.to_string()),
        other => Status::data_loss(other.to_string()),
    }
}

impl SasFlightService {
    /// Service sur `root` (le dossier doit exister).
    ///
    /// # Errors
    /// Dossier absent ou illisible.
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = std::fs::canonicalize(root.as_ref())?;
        if !root.is_dir() {
            return Err(std::io::Error::other(
                "le dossier racine n'est pas un dossier",
            ));
        }
        Ok(Self {
            root: Arc::new(root),
            slots: Arc::new(Semaphore::new(ServiceLimits::default().concurrent_requests)),
            limits: ServiceLimits::default(),
            stats: Arc::new(Stats::default()),
            default_partitions: std::thread::available_parallelism().map_or(4, usize::from),
        })
    }

    /// Set limits before sharing the service with clients.
    ///
    /// # Errors
    /// Zero/invalid limits or a timeout above one hour.
    pub fn with_limits(mut self, limits: ServiceLimits) -> std::io::Result<Self> {
        limits.validate()?;
        self.slots = Arc::new(Semaphore::new(limits.concurrent_requests));
        self.limits = limits;
        Ok(self)
    }

    fn budget(&self) -> Result<RequestBudget, Status> {
        RequestBudget::acquire(&self.slots, self.limits.request_timeout)
    }

    fn check_payload(&self, len: usize) -> Result<(), Status> {
        if len > self.limits.request_bytes {
            return Err(Status::resource_exhausted("requête trop volumineuse"));
        }
        Ok(())
    }

    fn check_response(&self, len: usize) -> Result<(), Status> {
        if len > self.limits.response_bytes {
            return Err(Status::resource_exhausted(
                "métadonnées de réponse trop volumineuses",
            ));
        }
        Ok(())
    }

    /// Nombre d'endpoints par défaut quand le descripteur n'en demande pas.
    #[must_use]
    pub fn with_default_partitions(mut self, n: usize) -> Self {
        self.default_partitions = n.clamp(1, MAX_PARTITIONS);
        self
    }

    /// Compteurs d'activité.
    #[must_use]
    pub fn stats(&self) -> Arc<Stats> {
        Arc::clone(&self.stats)
    }

    /// Chemin d'un fichier confiné au dossier racine (ni `..`, ni chemin absolu, ni lien sortant).
    fn resolve(&self, rel: &str) -> Result<PathBuf, Status> {
        if rel.is_empty() || rel.contains('\0') {
            return Err(invalid("chemin vide ou invalide"));
        }
        let p = Path::new(rel);
        if p.is_absolute()
            || p.components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err(invalid(
                "chemin relatif au dossier racine attendu, sans « .. »",
            ));
        }
        if p.extension().and_then(|e| e.to_str()) != Some("sas7bdat") {
            return Err(invalid("seuls les fichiers .sas7bdat sont servis"));
        }
        let full = std::fs::canonicalize(self.root.join(p))
            .map_err(|e| Status::not_found(format!("{rel} : {e}")))?;
        if !full.starts_with(self.root.as_path()) || !full.is_file() {
            return Err(Status::not_found(format!("{rel} : fichier inconnu")));
        }
        Ok(full)
    }

    #[allow(clippy::field_reassign_with_default)]
    fn read_options(&self, spec: &ReadSpec) -> Result<ReadOptions, Status> {
        let mut o = ReadOptions::default();
        o.max_batch_bytes = self.limits.batch_bytes;
        o.max_output_columns = self.limits.output_columns;
        o.group_bytes = 1024 * 1024;
        o.missing = match spec.missing.as_deref() {
            None | Some("null") => MissingMode::Null,
            Some("sidecar") => MissingMode::Sidecar,
            Some(s) => return Err(invalid(format!("missing = {s:?} : null ou sidecar"))),
        };
        o.dates = match spec.dates.as_deref() {
            None | Some("native") => DateMode::Native,
            Some("float") => DateMode::AsFloat,
            Some(s) => return Err(invalid(format!("dates = {s:?} : native ou float"))),
        };
        o.trailing_blanks = match spec.trailing_blanks.as_deref() {
            None | Some("trim") => TrailingBlanks::Trim,
            Some("keep") => TrailingBlanks::Keep,
            Some(s) => return Err(invalid(format!("trailing_blanks = {s:?} : trim ou keep"))),
        };
        o.plan = match spec.plan.as_deref() {
            None | Some("fast") => PlanMode::Fast,
            Some("full") => PlanMode::Full,
            Some(s) => return Err(invalid(format!("plan = {s:?} : fast ou full"))),
        };
        if let Some(label) = &spec.encoding {
            o.encoding_override = Some(
                Encoding::for_label(label.as_bytes())
                    .ok_or_else(|| invalid(format!("encodage {label:?} inconnu")))?,
            );
        }
        if let Some(n) = spec.batch_rows {
            o.batch_rows = n.clamp(1, 1 << 20);
        }
        Ok(o)
    }

    fn open(
        &self,
        path: &Path,
        opts: &ReadOptions,
    ) -> Result<Arc<SasFile<CountingSource<FileSource>>>, Status> {
        let src = FileSource::open(path).map_err(|e| Status::not_found(e.to_string()))?;
        let mut open = opts.open_options();
        open.limits.max_columns = self.limits.output_columns.min(u32::MAX as usize) as u32;
        open.limits.max_page_length = 1024 * 1024;
        open.limits.max_row_length = 1024 * 1024;
        open.limits.max_text_bytes = 4 * 1024 * 1024;
        let file =
            SasFile::open(CountingSource::new(src), open).map_err(|e| map_error(e.into()))?;
        Ok(Arc::new(file))
    }

    fn projection(
        &self,
        schema: &SchemaRef,
        columns: Option<&Vec<String>>,
    ) -> Result<Option<Vec<usize>>, Status> {
        if columns.map_or(schema.fields().len(), Vec::len) > self.limits.output_columns {
            return Err(Status::resource_exhausted("trop de colonnes de sortie"));
        }
        let Some(cols) = columns else { return Ok(None) };
        let mut seen = std::collections::HashSet::with_capacity(cols.len());
        if cols.iter().any(|c| !seen.insert(c)) {
            return Err(invalid("les colonnes répétées ne sont pas autorisées"));
        }
        cols.iter()
            .map(|c| {
                schema
                    .index_of(c)
                    .map_err(|_| invalid(format!("colonne {c:?} absente de la table")))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    fn descriptor_command(&self, d: &FlightDescriptor) -> Result<Command, Status> {
        use arrow_flight::flight_descriptor::DescriptorType;
        let path_bytes = d
            .path
            .iter()
            .try_fold(0usize, |total, p| {
                total.checked_add(p.len().saturating_add(8))
            })
            .ok_or_else(|| Status::resource_exhausted("chemin trop volumineux"))?;
        self.check_payload(d.cmd.len().saturating_add(path_bytes))?;
        match DescriptorType::try_from(d.r#type) {
            Ok(DescriptorType::Path) => {
                let path = d
                    .path
                    .first()
                    .ok_or_else(|| invalid("descripteur PATH sans élément"))?
                    .clone();
                Ok(Command {
                    path,
                    ..Command::default()
                })
            }
            Ok(DescriptorType::Cmd) => serde_json::from_slice(&d.cmd)
                .map_err(|e| invalid(format!("commande JSON invalide : {e}"))),
            _ => Err(invalid("descripteur de type inconnu")),
        }
    }

    /// Schéma, nombre de lignes et endpoints d'un fichier, sans lire de page de données.
    fn info(&self, cmd: &Command, descriptor: FlightDescriptor) -> Result<FlightInfo, Status> {
        let path = self.resolve(&cmd.path)?;
        let opts = self.read_options(&cmd.spec)?;
        let file = self.open(&path, &opts)?;
        let meta = file.meta();
        let schema = arrow_schema(meta, &opts);
        let projection = self.projection(&schema, cmd.spec.columns.as_ref())?;
        let out_schema: SchemaRef = match &projection {
            Some(p) => Arc::new(schema.project(p).map_err(internal)?),
            None => Arc::clone(&schema),
        };
        let plan = file.page_plan(opts.plan).map_err(|e| map_error(e.into()))?;
        let parts = match cmd.spec.limit {
            Some(_) => 1,
            None => cmd
                .partitions
                .unwrap_or(self.default_partitions)
                .clamp(1, MAX_PARTITIONS),
        };
        let groups = plan.split(parts);
        let visible = meta.visible_rows();
        let rows_hint = cmd.spec.limit.map_or(visible, |l| l.min(visible));
        let bytes = meta
            .header
            .page_count
            .saturating_mul(u64::from(meta.header.page_length));
        let mut info = FlightInfo::new()
            .try_with_schema(&out_schema)
            .map_err(internal)?
            .with_descriptor(descriptor)
            .with_total_records(i64::try_from(rows_hint).unwrap_or(-1))
            .with_total_bytes(i64::try_from(bytes).unwrap_or(-1))
            // exact seulement sans ligne supprimée (ADR 0004)
            .with_ordered(true);
        self.check_response(info.encoded_len())?;
        for g in &groups {
            let body = TicketBody {
                path: cmd.path.clone(),
                pages: (g.pages.start, g.pages.end),
                spec: cmd.spec.clone(),
            };
            let ticket = serde_json::to_vec(&body).map_err(internal)?;
            info = info.with_endpoint(FlightEndpoint::new().with_ticket(Ticket::new(ticket)));
            self.check_response(info.encoded_len())?;
        }
        self.stats
            .info_bytes
            .fetch_add(file.source().bytes_read(), Ordering::Relaxed);
        self.stats.info_calls.fetch_add(1, Ordering::Relaxed);
        Ok(info)
    }
}

type Stream<T> = BoxStream<'static, Result<T, Status>>;

fn bearer_ok(token: &str, req_token: Option<&str>) -> bool {
    req_token.is_some_and(|h| {
        h.strip_prefix("Bearer ").is_some_and(|t| {
            !token.is_empty() && constant_time_eq::constant_time_eq(t.as_bytes(), token.as_bytes())
        })
    })
}

/// Intercepteur d'authentification : jeton porteur (`authorization: Bearer <jeton>`).
pub fn bearer_interceptor(
    token: String,
) -> impl Fn(Request<()>) -> Result<Request<()>, Status> + Clone {
    move |req: Request<()>| {
        let header = req
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok());
        if bearer_ok(&token, header) {
            Ok(req)
        } else {
            Err(Status::unauthenticated("jeton porteur absent ou invalide"))
        }
    }
}

#[tonic::async_trait]
impl FlightService for SasFlightService {
    type HandshakeStream = Stream<HandshakeResponse>;
    type ListFlightsStream = Stream<FlightInfo>;
    type DoGetStream = Stream<FlightData>;
    type DoPutStream = Stream<PutResult>;
    type DoActionStream = Stream<arrow_flight::Result>;
    type ListActionsStream = Stream<ActionType>;
    type DoExchangeStream = Stream<FlightData>;

    async fn handshake(
        &self,
        _request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        // l'authentification est faite par l'intercepteur (jeton porteur) ou à l'amont
        Ok(Response::new(futures::stream::empty().boxed()))
    }

    async fn list_flights(
        &self,
        request: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        let budget = self.budget()?;
        self.check_payload(request.get_ref().expression.len())?;
        let filter = String::from_utf8_lossy(&request.get_ref().expression).into_owned();
        let svc = self.clone();
        let deadline = budget.deadline;
        let infos = budget
            .blocking(move || {
                let mut out = Vec::new();
                let mut response_bytes = 0usize;
                let mut stack = vec![(*svc.root).clone()];
                let mut visited = 0usize;
                while let Some(dir) = stack.pop() {
                    let Ok(rd) = std::fs::read_dir(&dir) else {
                        continue;
                    };
                    for entry in rd {
                        visited += 1;
                        if visited > svc.limits.listing_entries {
                            return Err(Status::resource_exhausted("listing trop volumineux"));
                        }
                        if Instant::now() >= deadline {
                            return Err(Status::deadline_exceeded("listing trop long"));
                        }
                        let Ok(e) = entry else {
                            continue;
                        };
                        let Ok(kind) = e.file_type() else {
                            continue;
                        };
                        if kind.is_symlink() {
                            continue;
                        }
                        let p = e.path();
                        if kind.is_dir() {
                            stack.push(p);
                        } else if kind.is_file()
                            && p.extension().and_then(|x| x.to_str()) == Some("sas7bdat")
                        {
                            let Ok(rel) = p.strip_prefix(svc.root.as_path()) else {
                                continue;
                            };
                            let rel = rel.to_string_lossy().into_owned();
                            if !filter.is_empty() && !rel.contains(&filter) {
                                continue;
                            }
                            let cmd = Command {
                                path: rel.clone(),
                                ..Command::default()
                            };
                            let mut info =
                                match svc.info(&cmd, FlightDescriptor::new_path(vec![rel])) {
                                    Ok(info) => info,
                                    Err(e)
                                        if matches!(
                                            e.code(),
                                            tonic::Code::ResourceExhausted
                                                | tonic::Code::DeadlineExceeded
                                        ) =>
                                    {
                                        return Err(e)
                                    }
                                    Err(_) => continue,
                                };
                            if out.len() >= svc.limits.listing_files {
                                return Err(Status::resource_exhausted(
                                    "trop de fichiers dans le listing",
                                ));
                            }
                            info.endpoint.clear();
                            response_bytes =
                                response_bytes.checked_add(info.encoded_len()).ok_or_else(
                                    || Status::resource_exhausted("listing trop volumineux"),
                                )?;
                            svc.check_response(response_bytes)?;
                            out.push(info);
                        }
                    }
                }
                Ok(out)
            })
            .await?;
        Ok(Response::new(budget.stream(
            futures::stream::iter(infos.into_iter().map(Ok)).boxed(),
        )))
    }

    async fn get_flight_info(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        let budget = self.budget()?;
        let descriptor = request.into_inner();
        let cmd = self.descriptor_command(&descriptor)?;
        let svc = self.clone();
        let info = budget.blocking(move || svc.info(&cmd, descriptor)).await?;
        Ok(Response::new(info))
    }

    async fn poll_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<PollInfo>, Status> {
        Err(Status::unimplemented("poll_flight_info"))
    }

    async fn get_schema(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<SchemaResult>, Status> {
        let budget = self.budget()?;
        let descriptor = request.into_inner();
        let cmd = self.descriptor_command(&descriptor)?;
        let svc = self.clone();
        let schema = budget
            .blocking(move || -> Result<SchemaRef, Status> {
                let path = svc.resolve(&cmd.path)?;
                let opts = svc.read_options(&cmd.spec)?;
                let file = svc.open(&path, &opts)?;
                let schema = arrow_schema(file.meta(), &opts);
                svc.stats
                    .info_bytes
                    .fetch_add(file.source().bytes_read(), Ordering::Relaxed);
                match svc.projection(&schema, cmd.spec.columns.as_ref())? {
                    Some(p) => Ok(Arc::new(schema.project(&p).map_err(internal)?)),
                    None => Ok(schema),
                }
            })
            .await?;
        // même encodage IPC que le schéma d'un `FlightInfo`
        let encoded = FlightInfo::new()
            .try_with_schema(&schema)
            .map_err(internal)?;
        self.check_response(encoded.encoded_len())?;
        Ok(Response::new(SchemaResult {
            schema: encoded.schema,
        }))
    }

    async fn do_get(
        &self,
        request: Request<Ticket>,
    ) -> Result<Response<Self::DoGetStream>, Status> {
        let budget = self.budget()?;
        self.check_payload(request.get_ref().ticket.len())?;
        let body: TicketBody = serde_json::from_slice(&request.get_ref().ticket)
            .map_err(|e| invalid(format!("ticket invalide : {e}")))?;
        let svc = self.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<RecordBatch, FlightError>>(1);
        // ouverture et validation avant le premier lot : une erreur de ticket est une erreur d'appel
        let (file, mut reader) = budget
            .blocking(move || -> Result<_, Status> {
                let path = svc.resolve(&body.path)?;
                let opts = svc.read_options(&body.spec)?;
                let file = svc.open(&path, &opts)?;
                let schema = arrow_schema(file.meta(), &opts);
                let projection = svc.projection(&schema, body.spec.columns.as_ref())?;
                let page_count = file.meta().header.page_count;
                let (start, end) = body.pages;
                if start > end || end > page_count {
                    return Err(invalid("plage de pages hors du fichier"));
                }
                let plan = Arc::new(file.page_plan(opts.plan).map_err(|e| map_error(e.into()))?);
                // rows_before et nombre de lignes viennent du plan, jamais du client
                let compressed = file.meta().compression != Compression::None;
                if compressed && (start, end) != (0, page_count) {
                    return Err(invalid("table compressée : lecture complète seulement"));
                }
                let rows_before = plan.rows_before(start).unwrap_or(0);
                let group = PageGroup {
                    pages: start..end,
                    rows_before,
                    rows: plan
                        .rows_before(end)
                        .map_or(0, |e| e.saturating_sub(rows_before)),
                };
                let reader = BatchReader::with_group(
                    Arc::clone(&file),
                    &group,
                    plan,
                    projection,
                    body.spec.limit,
                    opts,
                )
                .map_err(map_error)?;
                Ok((file, reader))
            })
            .await?;
        let schema = reader.schema();
        let stats = Arc::clone(&self.stats);
        stats.get_calls.fetch_add(1, Ordering::Relaxed);
        let producer_budget = budget.clone();
        let runtime = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let _permit = &producer_budget;
            while !tx.is_closed() && Instant::now() < producer_budget.deadline {
                let Some(batch) = reader.next() else {
                    break;
                };
                let item = batch.map_err(|e| FlightError::ExternalError(Box::new(e)));
                let failed = item.is_err();
                let sent = runtime.block_on(timeout_at(producer_budget.deadline, tx.send(item)));
                if !matches!(sent, Ok(Ok(()))) || failed {
                    break;
                }
            }
            stats
                .data_bytes
                .fetch_add(file.source().bytes_read(), Ordering::Relaxed);
        });
        let batches = tokio_stream::wrappers::ReceiverStream::new(rx);
        let flight = FlightDataEncoderBuilder::new()
            .with_schema(schema)
            .build(batches)
            .map_err(|e| {
                if matches!(&e, FlightError::ExternalError(inner)
                    if matches!(inner.downcast_ref::<uniformat_sas_arrow::Error>(), Some(uniformat_sas_arrow::Error::ResourceLimit(_)))) {
                    Status::resource_exhausted(e.to_string())
                } else {
                    Status::data_loss(e.to_string())
                }
            })
            .boxed();
        Ok(Response::new(budget.stream(flight)))
    }

    async fn do_put(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        Err(Status::unimplemented("lecture seule"))
    }

    async fn do_exchange(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoExchangeStream>, Status> {
        Err(Status::unimplemented("do_exchange"))
    }

    async fn do_action(
        &self,
        request: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        match request.get_ref().r#type.as_str() {
            // compteurs d'activité (octets lus pour les schémas et pour les données) : observabilité et tests
            "stats" => {
                let st = &self.stats;
                let body = serde_json::json!({
                    "info_bytes": st.info_bytes.load(Ordering::Relaxed),
                    "data_bytes": st.data_bytes.load(Ordering::Relaxed),
                    "info_calls": st.info_calls.load(Ordering::Relaxed),
                    "get_calls": st.get_calls.load(Ordering::Relaxed),
                });
                let out = arrow_flight::Result {
                    body: serde_json::to_vec(&body).map_err(internal)?.into(),
                };
                Ok(Response::new(futures::stream::iter([Ok(out)]).boxed()))
            }
            other => Err(Status::unimplemented(format!("action {other:?} inconnue"))),
        }
    }

    async fn list_actions(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::ListActionsStream>, Status> {
        let stats = ActionType {
            r#type: "stats".to_owned(),
            description: "compteurs d'activité (JSON) : octets lus pour les schémas et les données"
                .to_owned(),
        };
        Ok(Response::new(futures::stream::iter([Ok(stats)]).boxed()))
    }
}

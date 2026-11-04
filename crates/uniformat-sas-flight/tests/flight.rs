//! Critères de fin de J7 (`docs/roadmap.md`) côté Rust : schéma sans transfert de données, endpoints lus en parallèle =
//! lecture directe, tickets revalidés, jeton porteur, listing. Sauté sans `UNIFORMAT_CORPUS`.

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_flight::flight_service_server::FlightServiceServer;
use arrow_flight::{FlightClient, FlightDescriptor, Ticket};
use futures::TryStreamExt;
use tonic::transport::{Channel, Server};
use uniformat_sas_arrow::{arrow_schema, BatchReader, ReadOptions};
use uniformat_sas_flight::{
    bearer_interceptor, Command, ReadSpec, SasFlightService, Stats, TicketBody,
};
use uniformat_sas_io::{CountingSource, FileSource, OpenOptions, SasFile};
use uniformat_testkit::Corpus;

async fn serve(root: &std::path::Path, token: Option<&str>) -> (SocketAddr, Arc<Stats>) {
    let service = SasFlightService::new(root).expect("service");
    let stats = service.stats();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let token = token.map(str::to_owned);
    tokio::spawn(async move {
        let mut b = Server::builder();
        let _ = match token {
            Some(t) => {
                b.add_service(FlightServiceServer::with_interceptor(
                    service,
                    bearer_interceptor(t),
                ))
                .serve_with_incoming(incoming)
                .await
            }
            None => {
                b.add_service(FlightServiceServer::new(service))
                    .serve_with_incoming(incoming)
                    .await
            }
        };
    });
    (addr, stats)
}

async fn client(addr: SocketAddr) -> FlightClient {
    let channel = Channel::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    FlightClient::new(channel)
}

fn cmd(path: &str, partitions: usize, spec: ReadSpec) -> FlightDescriptor {
    let c = Command {
        path: path.to_owned(),
        partitions: Some(partitions),
        spec,
    };
    FlightDescriptor::new_cmd(serde_json::to_vec(&c).unwrap())
}

fn direct(path: &std::path::Path) -> Vec<RecordBatch> {
    let opts = ReadOptions::default();
    let file =
        Arc::new(SasFile::open(FileSource::open(path).unwrap(), opts.open_options()).unwrap());
    let pages = file.meta().header.page_count;
    BatchReader::new(file, 0..pages, None, None, opts)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn concat(batches: &[RecordBatch]) -> RecordBatch {
    arrow_select::concat::concat_batches(&batches[0].schema(), batches).unwrap()
}

async fn collect(c: &mut FlightClient, t: Ticket) -> Vec<RecordBatch> {
    c.do_get(t).await.unwrap().try_collect().await.unwrap()
}

#[tokio::test]
async fn schema_without_data_and_parallel_endpoints_equal_the_direct_read() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let root = corpus.root().join("files");
    let (addr, stats) = serve(&root, None).await;
    let mut c = client(addr).await;
    for (rel, parts) in [
        ("taille/taille_n50000_no.sas7bdat", 4usize),
        ("taille/taille_n50000_char.sas7bdat", 4),
        ("compression/compression_pat2_binary.sas7bdat", 4),
    ] {
        let path = root.join(rel);
        let before = stats.info_bytes.load(Ordering::Relaxed);
        let info = c
            .get_flight_info(cmd(rel, parts, ReadSpec::default()))
            .await
            .unwrap();
        let read = stats.info_bytes.load(Ordering::Relaxed) - before;
        assert_eq!(
            stats.data_bytes.load(Ordering::Relaxed),
            0,
            "{rel} : aucun octet de données avant DoGet"
        );
        // borne de J4 (critère 1) : en-tête + pages de métadonnées
        let probe = SasFile::open(
            CountingSource::new(FileSource::open(&path).unwrap()),
            OpenOptions::default(),
        )
        .unwrap();
        let meta_pages = probe
            .scanned_pages()
            .iter()
            .filter(|p| {
                !matches!(
                    p.kind,
                    uniformat_sas::PageKind::Data | uniformat_sas::PageKind::Compressed
                )
            })
            .count() as u64;
        let h = &probe.meta().header;
        let bound = u64::from(h.header_length) + meta_pages * u64::from(h.page_length);
        assert!(read <= bound, "{rel} : {read} octets lus > {bound}");
        assert_eq!(read, probe.source().bytes_read(), "{rel}");
        let schema = info.clone().try_decode_schema().unwrap();
        assert_eq!(
            &schema,
            arrow_schema(probe.meta(), &ReadOptions::default()).as_ref(),
            "{rel}"
        );
        let compressed = probe.meta().compression != uniformat_sas::Compression::None;
        if compressed {
            assert_eq!(
                info.endpoint.len(),
                1,
                "{rel} : table compressée, un endpoint"
            );
        } else {
            assert_eq!(info.endpoint.len(), parts, "{rel}");
        }
        assert_eq!(info.total_records, probe.meta().visible_rows() as i64);
        // les endpoints se lisent en parallèle ; leur concaténation (ordre des pages) = lecture directe
        let tickets: Vec<Ticket> = info
            .endpoint
            .iter()
            .map(|e| e.ticket.clone().unwrap())
            .collect();
        let parts_read = futures::future::join_all(tickets.into_iter().map(|t| async move {
            let mut c = client(addr).await;
            collect(&mut c, t).await
        }))
        .await;
        let got: Vec<RecordBatch> = parts_read.into_iter().flatten().collect();
        let (g, w) = (concat(&got), concat(&direct(&path)));
        assert_eq!(g.num_rows(), w.num_rows(), "{rel}");
        let data = |b: &RecordBatch| b.columns().iter().map(|a| a.to_data()).collect::<Vec<_>>();
        assert_eq!(data(&g), data(&w), "{rel} : lignes différentes");
        assert!(stats.data_bytes.load(Ordering::Relaxed) > 0);
        stats.data_bytes.store(0, Ordering::Relaxed);
    }
}

#[tokio::test]
async fn projection_limit_sidecar_and_schema_call() {
    let Some(corpus) = Corpus::from_env() else {
        return;
    };
    let root = corpus.root().join("files");
    let (addr, _) = serve(&root, None).await;
    let mut c = client(addr).await;
    let rel = "taille/taille_n50000_no.sas7bdat";
    let file = SasFile::open(
        FileSource::open(root.join(rel)).unwrap(),
        OpenOptions::default(),
    )
    .unwrap();
    let full = arrow_schema(file.meta(), &ReadOptions::default());
    let names: Vec<String> = full.fields().iter().map(|f| f.name().clone()).collect();
    let last = names[names.len() - 1].clone();
    let spec = ReadSpec {
        columns: Some(vec![last.clone(), names[0].clone()]),
        limit: Some(7),
        ..ReadSpec::default()
    };
    let info = c.get_flight_info(cmd(rel, 8, spec.clone())).await.unwrap();
    assert_eq!(info.endpoint.len(), 1, "LIMIT : un seul endpoint, exact");
    assert_eq!(info.total_records, 7);
    let batches = collect(&mut c, info.endpoint[0].ticket.clone().unwrap()).await;
    let got = concat(&batches);
    assert_eq!(got.num_rows(), 7);
    let got_names: Vec<String> = got
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().clone())
        .collect();
    assert_eq!(got_names, vec![last, names[0].clone()]);
    // GetSchema = schéma projeté
    let sr = c.get_schema(cmd(rel, 1, spec)).await.unwrap();
    assert_eq!(sr.fields().len(), 2);
    // manquants spéciaux : 28 codes
    let spec = ReadSpec {
        missing: Some("sidecar".into()),
        ..ReadSpec::default()
    };
    let info = c
        .get_flight_info(cmd("valeurs/valeurs_manquants_no.sas7bdat", 2, spec))
        .await
        .unwrap();
    let mut codes = Vec::new();
    for e in &info.endpoint {
        for b in collect(&mut c, e.ticket.clone().unwrap()).await {
            let idx = b.schema().index_of("x__missing").unwrap();
            let a = b
                .column(idx)
                .as_any()
                .downcast_ref::<arrow_array::UInt8Array>()
                .unwrap()
                .clone();
            codes.extend(a.iter().flatten());
        }
    }
    assert_eq!(codes, (1u8..=28).collect::<Vec<_>>());
}

#[tokio::test]
async fn tickets_and_paths_are_validated_and_listing_works() {
    let Some(corpus) = Corpus::from_env() else {
        return;
    };
    let root = corpus.root().join("files");
    let (addr, _) = serve(&root, None).await;
    let mut c = client(addr).await;
    for bad in [
        "../MANIFEST.md",
        "/etc/passwd",
        "taille/../../MANIFEST.sas7bdat",
        "base/base_n100_mix_no.csv",
        "",
        "absent.sas7bdat",
    ] {
        assert!(
            c.get_flight_info(cmd(bad, 1, ReadSpec::default()))
                .await
                .is_err(),
            "{bad}"
        );
    }
    // ticket forgé : plage hors du fichier, plage inversée, chemin hors racine, JSON invalide
    let forged = |pages: (u64, u64), path: &str| {
        Ticket::new(
            serde_json::to_vec(&TicketBody {
                path: path.into(),
                pages,
                spec: ReadSpec::default(),
            })
            .unwrap(),
        )
    };
    assert!(c
        .do_get(forged((0, 1_000_000), "base/base_n100_mix_no.sas7bdat"))
        .await
        .is_err());
    assert!(c
        .do_get(forged((5, 2), "base/base_n100_mix_no.sas7bdat"))
        .await
        .is_err());
    assert!(c.do_get(forged((0, 1), "../x.sas7bdat")).await.is_err());
    assert!(c
        .do_get(Ticket::new(b"pas du json".to_vec()))
        .await
        .is_err());
    // table compressée : plage partielle refusée
    assert!(c
        .do_get(forged((1, 2), "compression/compression_pat2_char.sas7bdat"))
        .await
        .is_err());
    // listing
    let infos: Vec<_> = c
        .list_flights(Vec::<u8>::new())
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    let on_disk = walk_count(&root);
    assert_eq!(
        infos.len(),
        on_disk,
        "tous les fichiers du dossier sont listés"
    );
    assert!(infos.len() >= 190);
    assert!(infos
        .iter()
        .all(|i| i.endpoint.is_empty() && i.flight_descriptor.is_some()));
    let only: Vec<_> = c
        .list_flights(b"valeurs_manquants".to_vec())
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    assert!(!only.is_empty() && only.len() < 10);
}

#[tokio::test]
async fn bearer_token_is_required_when_configured() {
    let Some(corpus) = Corpus::from_env() else {
        return;
    };
    let root = corpus.root().join("files");
    let (addr, _) = serve(&root, Some("secret")).await;
    let d = cmd("base/base_n100_mix_no.sas7bdat", 1, ReadSpec::default());
    let mut anon = client(addr).await;
    assert!(anon.get_flight_info(d.clone()).await.is_err());
    let mut wrong = client(addr).await;
    wrong.add_header("authorization", "Bearer autre").unwrap();
    assert!(wrong.get_flight_info(d.clone()).await.is_err());
    let mut ok = client(addr).await;
    ok.add_header("authorization", "Bearer secret").unwrap();
    assert_eq!(ok.get_flight_info(d).await.unwrap().total_records, 100);
}

fn walk_count(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                walk_count(&p)
            } else {
                usize::from(p.extension().and_then(|x| x.to_str()) == Some("sas7bdat"))
            }
        })
        .sum()
}

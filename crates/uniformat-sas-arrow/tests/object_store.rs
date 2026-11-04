//! Critère 4 de J4 : lecture par `object_store` (magasin local et serveur HTTP de test) identique à la lecture
//! synchrone, avec un nombre de requêtes borné (≤ 2 + groupes de pages). Sauté sans `UNIFORMAT_CORPUS`.
#![cfg(feature = "object-store")]

use std::path::Path as FsPath;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use arrow_array::RecordBatch;
use futures::TryStreamExt;
use object_store::http::HttpBuilder;
use object_store::local::LocalFileSystem;
use object_store::path::Path;
use object_store::{ClientOptions, ObjectStore};
use uniformat_sas_arrow::{BatchReader, ObjectFile, ReadOptions};
use uniformat_sas_io::{FileSource, SasFile};
use uniformat_testkit::Corpus;
#[path = "support/http.rs"]
mod http;

/// Empreinte des lots : lignes et somme de la première colonne numérique (ordre des lignes compris).
fn fingerprint(batches: &[RecordBatch]) -> (usize, String) {
    let rows = batches.iter().map(RecordBatch::num_rows).sum();
    let mut acc = String::new();
    for b in batches {
        let col = b.column(0);
        for i in 0..b.num_rows() {
            if let Some(a) = col.as_primitive_opt::<Float64Type>() {
                acc.push_str(&format!("{:?};", a.value(i)));
            } else if let Some(s) = col.as_string_opt::<i32>() {
                acc.push_str(s.value(i));
                acc.push(';');
            }
        }
    }
    (rows, acc)
}

fn sync_batches(path: &FsPath, ro: &ReadOptions) -> Vec<RecordBatch> {
    let file = Arc::new(
        SasFile::open(FileSource::open(path).expect("source"), ro.open_options()).expect("open"),
    );
    let pages = 0..file.meta().header.page_count;
    BatchReader::new(file, pages, None, None, ro.clone())
        .expect("lecteur")
        .collect::<Result<_, _>>()
        .expect("lots")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
async fn local_object_store_matches_sync_reader() {
    let corpus = Corpus::require_from_env();
    let store: Arc<dyn ObjectStore> =
        Arc::new(LocalFileSystem::new_with_prefix(corpus.root()).expect("magasin"));
    for name in [
        "files/taille/taille_n50000_no.sas7bdat",
        "files/taille/taille_n50000_char.sas7bdat",
        "files/taille/taille_n50000_binary.sas7bdat",
        "files/suppression/suppression_n100_del10_no.sas7bdat",
        "files/taille/taille_wide300_no.sas7bdat",
    ] {
        let ro = ReadOptions {
            batch_rows: 4096,
            group_bytes: 64 * 1024,
            ..ReadOptions::default()
        };
        let want = fingerprint(&sync_batches(&corpus.root().join(name), &ro));
        let obj = ObjectFile::open(Arc::clone(&store), Path::from(name), ro.clone())
            .await
            .expect("ouverture");
        assert_eq!(obj.schema().fields().len(), obj.meta().columns.len());
        // séquentiel
        let got: Vec<RecordBatch> = obj
            .stream_all(None, None)
            .expect("flux")
            .try_collect()
            .await
            .expect("lots");
        assert_eq!(fingerprint(&got), want, "{name} séquentiel");
        // partitions (plan rapide et plan complet), lues dans l'ordre des groupes
        for plan_mode in [
            uniformat_sas_io::PlanMode::Fast,
            uniformat_sas_io::PlanMode::Full,
        ] {
            let obj = ObjectFile::open(
                Arc::clone(&store),
                Path::from(name),
                ReadOptions {
                    plan: plan_mode,
                    ..ro.clone()
                },
            )
            .await
            .expect("ouverture");
            let plan = Arc::new(obj.page_plan().await.expect("plan"));
            for parts in [1usize, 4, 8] {
                let mut all = Vec::new();
                for g in plan.split(parts) {
                    let part: Vec<RecordBatch> = obj
                        .stream(&g, Some(Arc::clone(&plan)), None, None)
                        .expect("flux")
                        .try_collect()
                        .await
                        .expect("lots");
                    all.extend(part);
                }
                assert_eq!(
                    fingerprint(&all),
                    want,
                    "{name} {plan_mode:?} {parts} partitions"
                );
            }
        }
        // LIMIT
        let limited: Vec<RecordBatch> = obj
            .stream_all(None, Some(10))
            .expect("flux")
            .try_collect()
            .await
            .expect("lots");
        assert_eq!(
            limited.iter().map(RecordBatch::num_rows).sum::<usize>(),
            10.min(want.0)
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
async fn http_range_requests_are_bounded() {
    let corpus = Corpus::require_from_env();
    let requests = Arc::new(AtomicU64::new(0));
    let addr = http::serve(corpus.root().to_path_buf(), Arc::clone(&requests)).await;
    let store: Arc<dyn ObjectStore> = Arc::new(
        HttpBuilder::new()
            .with_url(format!("http://{addr}"))
            .with_client_options(ClientOptions::new().with_allow_http(true))
            .build()
            .expect("client HTTP"),
    );
    let name = "files/taille/taille_n50000_no.sas7bdat";
    let ro = ReadOptions {
        batch_rows: 4096,
        group_bytes: 128 * 1024,
        ..ReadOptions::default()
    };
    let want = fingerprint(&sync_batches(&corpus.root().join(name), &ro));
    // schéma seul : head + préfixe, aucune page de données demandée au-delà
    let obj = ObjectFile::open(Arc::clone(&store), Path::from(name), ro.clone())
        .await
        .expect("ouverture");
    let for_schema = requests.load(Ordering::SeqCst);
    assert!(for_schema <= 2, "schéma : {for_schema} requêtes");
    // lecture complète
    let got: Vec<RecordBatch> = obj
        .stream_all(None, None)
        .expect("flux")
        .try_collect()
        .await
        .expect("lots");
    assert_eq!(fingerprint(&got), want);
    let h = &obj.meta().header;
    let total_pages = h.page_count;
    let per_group = (ro.group_bytes / u64::from(h.page_length)).max(1);
    let groups = total_pages.div_ceil(per_group);
    let used = requests.load(Ordering::SeqCst);
    eprintln!("HTTP : {used} requêtes pour {groups} groupes de pages ({total_pages} pages)");
    assert!(used <= 2 + groups, "{used} requêtes > 2 + {groups}");
    // lecture parallèle de 4 partitions : mêmes lignes, requêtes ≤ 2 + groupes
    requests.store(0, Ordering::SeqCst);
    let obj = ObjectFile::open(Arc::clone(&store), Path::from(name), ro.clone())
        .await
        .expect("ouverture");
    let plan = Arc::new(obj.page_plan().await.expect("plan"));
    let mut all = Vec::new();
    let mut part_groups = 0u64;
    for g in plan.split(4) {
        part_groups += (g.pages.end - g.pages.start).div_ceil(per_group);
        let part: Vec<RecordBatch> = obj
            .stream(&g, Some(Arc::clone(&plan)), None, None)
            .expect("flux")
            .try_collect()
            .await
            .expect("lots");
        all.extend(part);
    }
    assert_eq!(fingerprint(&all), want);
    let used = requests.load(Ordering::SeqCst);
    eprintln!("HTTP, 4 partitions : {used} requêtes pour {part_groups} groupes");
    assert!(
        used <= 2 + part_groups,
        "{used} requêtes > 2 + {part_groups}"
    );
}

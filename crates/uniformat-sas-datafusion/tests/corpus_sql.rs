//! Critères de fin de J4 côté DataFusion (`docs/roadmap.md`) : sur tout le corpus, `SELECT *`, `SELECT col`,
//! `COUNT(*)` et `LIMIT n` redonnent les valeurs du lecteur Rust direct ; 1, 4 et 8 partitions donnent le même
//! ensemble de lignes ; `COUNT(*)` ne lit aucune page de données ; lecture par HTTP identique. Sauté sans
//! `UNIFORMAT_CORPUS`.

use std::collections::BTreeMap;
use std::path::Path as FsPath;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use datafusion::arrow::array::{Array, RecordBatch};
use datafusion::arrow::util::display::array_value_to_string;
use datafusion::physical_plan::ExecutionPlanProperties;
use datafusion::prelude::{SessionConfig, SessionContext};
use object_store::http::HttpBuilder;
use object_store::path::Path;
use object_store::{ClientOptions, ObjectStore};
use uniformat_sas_arrow::{BatchReader, MissingMode, ReadOptions};
use uniformat_sas_datafusion::SasTableProvider;
use uniformat_sas_io::{FileSource, OpenOptions, SasFile};
use uniformat_testkit::Corpus;

#[path = "../../uniformat-sas-arrow/tests/support/http.rs"]
mod http;

/// Lignes sous forme de chaînes, comptées (multi-ensemble) : l'ordre des partitions n'est pas garanti.
fn multiset(batches: &[RecordBatch]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for b in batches {
        for i in 0..b.num_rows() {
            let row: Vec<String> = b
                .columns()
                .iter()
                .map(|c| {
                    if c.is_null(i) {
                        "∅".to_owned()
                    } else {
                        array_value_to_string(c, i).unwrap_or_default()
                    }
                })
                .collect();
            *m.entry(row.join("\u{1f}")).or_insert(0) += 1;
        }
    }
    m
}

fn rows(batches: &[RecordBatch]) -> usize {
    batches.iter().map(RecordBatch::num_rows).sum()
}

fn open_options_for(path: &FsPath) -> OpenOptions {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let dataset_b = stem
        .strip_prefix("test")
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| (16..=21).contains(&n));
    let mut o = OpenOptions::default();
    if path.to_string_lossy().contains("datareader") && dataset_b {
        o.encoding_override = Some(uniformat_sas::UTF_8);
    }
    o
}

fn direct(path: &FsPath, ro: &ReadOptions, projection: Option<Vec<usize>>) -> Vec<RecordBatch> {
    let file = Arc::new(
        SasFile::open(FileSource::open(path).expect("source"), ro.open_options()).expect("open"),
    );
    let pages = 0..file.meta().header.page_count;
    BatchReader::new(file, pages, projection, None, ro.clone())
        .expect("lecteur")
        .collect::<Result<_, _>>()
        .expect("lots")
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

async fn query(ctx: &SessionContext, sql: &str) -> Vec<RecordBatch> {
    ctx.sql(sql)
        .await
        .unwrap_or_else(|e| panic!("{sql} : {e}"))
        .collect()
        .await
        .unwrap_or_else(|e| panic!("{sql} : {e}"))
}

fn http_store(addr: std::net::SocketAddr) -> Arc<dyn ObjectStore> {
    Arc::new(
        HttpBuilder::new()
            .with_url(format!("http://{addr}"))
            .with_client_options(ClientOptions::new().with_allow_http(true))
            .build()
            .expect("client HTTP"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
async fn sql_equals_direct_reader_on_the_whole_corpus() {
    let corpus = Corpus::require_from_env();
    let (mut files, mut failures) = (0usize, Vec::new());
    for f in corpus.files().expect("corpus") {
        let open = open_options_for(&f.path);
        let ro = ReadOptions {
            encoding_override: open.encoding_override,
            ..ReadOptions::default()
        };
        let provider = match SasTableProvider::try_new_local(&f.path, ro.clone()).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let ncols = provider.meta().columns.len();
        let visible = provider.meta().visible_rows();
        let ctx = SessionContext::new_with_config(SessionConfig::new().with_target_partitions(4));
        ctx.register_table("t", Arc::new(provider)).expect("table");
        files += 1;
        let name = f.stem();
        // SELECT *
        let got = query(&ctx, "SELECT * FROM t").await;
        let want = direct(&f.path, &ro, None);
        if multiset(&got) != multiset(&want) {
            failures.push(format!("{name} : SELECT * ≠ lecteur direct"));
            continue;
        }
        // SELECT col (première et dernière colonne)
        let schema = want.first().map(RecordBatch::schema);
        if let Some(schema) = schema {
            for idx in [0, ncols.saturating_sub(1)] {
                let col = schema.field(idx).name().clone();
                let got = query(&ctx, &format!("SELECT {} FROM t", quote(&col))).await;
                let want = direct(&f.path, &ro, Some(vec![idx]));
                if multiset(&got) != multiset(&want) {
                    failures.push(format!("{name} : SELECT {col} ≠ lecteur direct"));
                }
            }
        }
        // COUNT(*)
        let c = query(&ctx, "SELECT COUNT(*) FROM t").await;
        let n = array_value_to_string(c[0].column(0), 0).unwrap_or_default();
        if n != visible.to_string() {
            failures.push(format!("{name} : COUNT(*) = {n}, attendu {visible}"));
        }
        // LIMIT
        let l = query(&ctx, "SELECT * FROM t LIMIT 7").await;
        let expected = usize::try_from(visible.min(7)).unwrap_or(7);
        if rows(&l) != expected {
            failures.push(format!("{name} : LIMIT 7 → {} lignes", rows(&l)));
        }
        let all = multiset(&want);
        for k in multiset(&l).keys() {
            if !all.contains_key(k) {
                failures.push(format!("{name} : LIMIT rend une ligne inconnue"));
                break;
            }
        }
    }
    eprintln!("{files} fichiers interrogés en SQL");
    assert!(files >= 225, "{files} fichiers");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Critère 3 : 1, 4 et 8 partitions → mêmes lignes ; le plan annonce bien le parallélisme.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
async fn partitions_give_the_same_rows() {
    let corpus = Corpus::require_from_env();
    for name in [
        "taille/taille_n50000_no",
        "suppression/suppression_n100_del10_no",
    ] {
        let path = corpus.root().join(format!("files/{name}.sas7bdat"));
        let ro = ReadOptions::default();
        let want = multiset(&direct(&path, &ro, None));
        for parts in [1usize, 4, 8] {
            let provider = SasTableProvider::try_new_local(&path, ro.clone())
                .await
                .expect("table")
                .with_target_partitions(parts);
            let ctx = SessionContext::new();
            ctx.register_table("t", Arc::new(provider)).expect("table");
            let got = query(&ctx, "SELECT * FROM t").await;
            assert_eq!(multiset(&got), want, "{name} {parts} partitions");
            let plan = ctx
                .sql("SELECT * FROM t")
                .await
                .expect("plan")
                .create_physical_plan()
                .await
                .expect("physique");
            let n = plan.output_partitioning().partition_count();
            eprintln!("{name} : {parts} partitions demandées, {n} dans le plan");
            assert!((1..=parts).contains(&n));
            if name.contains("n50000") && parts > 1 {
                assert!(n > 1, "le plan doit être parallèle sur 50 000 lignes");
            }
        }
    }
}

/// Critère 2 (fin) : `COUNT(*)` n'ouvre aucune page de données (HTTP : seulement `head` et le préfixe), et la
/// colonne annexe `x__missing` se filtre en SQL.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
async fn count_star_reads_no_data_pages_and_sidecar_filters_in_sql() {
    let corpus = Corpus::require_from_env();
    let requests = Arc::new(AtomicU64::new(0));
    let addr = http::serve(corpus.root().to_path_buf(), Arc::clone(&requests)).await;
    let store = http_store(addr);
    // COUNT(*) sur 50 000 lignes
    let provider = SasTableProvider::try_new(
        Arc::clone(&store),
        Path::from("files/taille/taille_n50000_no.sas7bdat"),
        ReadOptions::default(),
    )
    .await
    .expect("table");
    let ctx = SessionContext::new();
    ctx.register_table("t", Arc::new(provider)).expect("table");
    let opened = requests.load(Ordering::SeqCst);
    assert!(opened <= 2, "ouverture : {opened} requêtes");
    let c = query(&ctx, "SELECT COUNT(*) FROM t").await;
    assert_eq!(array_value_to_string(c[0].column(0), 0).unwrap(), "50000");
    assert_eq!(
        requests.load(Ordering::SeqCst),
        opened,
        "COUNT(*) a lu des pages de données"
    );
    // colonne annexe filtrée en SQL : `.A` est le code 3
    let ro = ReadOptions {
        missing: MissingMode::Sidecar,
        ..ReadOptions::default()
    };
    let provider = SasTableProvider::try_new(
        store,
        Path::from("files/valeurs/valeurs_manquants_no.sas7bdat"),
        ro,
    )
    .await
    .expect("table");
    let ctx = SessionContext::new();
    ctx.register_table("m", Arc::new(provider)).expect("table");
    let c = query(&ctx, "SELECT COUNT(*) FROM m WHERE x__missing = 3").await;
    assert_eq!(array_value_to_string(c[0].column(0), 0).unwrap(), "1");
    let c = query(&ctx, "SELECT COUNT(*) FROM m WHERE x IS NULL").await;
    assert_eq!(array_value_to_string(c[0].column(0), 0).unwrap(), "28");
    let c = query(&ctx, "SELECT x__missing FROM m ORDER BY x__missing").await;
    let codes: Vec<String> = (0..c[0].num_rows())
        .map(|i| array_value_to_string(c[0].column(0), i).unwrap())
        .collect();
    assert_eq!(codes, (1..=28).map(|n| n.to_string()).collect::<Vec<_>>());
}

/// Critère 4 : lecture HTTP identique à la lecture locale, requêtes bornées.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
async fn http_sql_matches_local() {
    let corpus = Corpus::require_from_env();
    let requests = Arc::new(AtomicU64::new(0));
    let addr = http::serve(corpus.root().to_path_buf(), Arc::clone(&requests)).await;
    let store = http_store(addr);
    let name = "files/taille/taille_n50000_char.sas7bdat";
    let ro = ReadOptions {
        group_bytes: 256 * 1024,
        ..ReadOptions::default()
    };
    let want = multiset(&direct(&corpus.root().join(name), &ro, None));
    let provider = SasTableProvider::try_new(store, Path::from(name), ro.clone())
        .await
        .expect("table")
        .with_target_partitions(4);
    let h = provider.meta().header.clone();
    let ctx = SessionContext::new();
    ctx.register_table("t", Arc::new(provider)).expect("table");
    let got = query(&ctx, "SELECT * FROM t").await;
    assert_eq!(multiset(&got), want);
    let per_group = (ro.group_bytes / u64::from(h.page_length)).max(1);
    // 4 partitions : au plus un groupe incomplet de plus par partition
    let bound = 2 + h.page_count.div_ceil(per_group) + 4;
    let used = requests.load(Ordering::SeqCst);
    eprintln!(
        "HTTP, SQL 4 partitions : {used} requêtes (borne {bound}, {} pages)",
        h.page_count
    );
    assert!(used <= bound, "{used} requêtes > {bound}");
}

//! Mandatory SQL and HTTP coverage for the independent object_store adapter.
#[path = "../../../tests/support/fixtures.rs"]
mod fixtures;
#[path = "../../uniformat-sas-arrow/tests/support/http.rs"]
mod http;
use datafusion::arrow::array::Float64Array;
use datafusion::prelude::SessionContext;
use object_store::{http::HttpBuilder, path::Path, ClientOptions};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use uniformat_sas::Compression;
use uniformat_sas_datafusion::{ReadOptions, SasTableProvider};

#[tokio::test]
async fn local_and_http_sql_match_on_generated_compressed_and_plain_files() {
    let dir = uniformat_testkit::tempdir().unwrap();
    let requests = Arc::new(AtomicU64::new(0));
    let addr = http::serve(dir.path().to_path_buf(), Arc::clone(&requests)).await;
    let store = Arc::new(
        HttpBuilder::new()
            .with_url(format!("http://{addr}"))
            .with_client_options(ClientOptions::new().with_allow_http(true))
            .build()
            .unwrap(),
    );
    for (i, compression) in [Compression::None, Compression::Char, Compression::Binary]
        .into_iter()
        .enumerate()
    {
        let name = format!("table{i}.sas7bdat");
        let file = dir.path().join(&name);
        std::fs::write(&file, fixtures::table(1000, compression)).unwrap();
        let local = SasTableProvider::try_new_local(&file, ReadOptions::default())
            .await
            .unwrap();
        let remote =
            SasTableProvider::try_new(store.clone(), Path::from(name), ReadOptions::default())
                .await
                .unwrap();
        let context = SessionContext::new();
        context
            .register_table("local_data", Arc::new(local))
            .unwrap();
        context
            .register_table("remote_data", Arc::new(remote))
            .unwrap();
        let mut results = Vec::new();
        for table in ["local_data", "remote_data"] {
            let sql = format!("SELECT x, text FROM {table} WHERE x < 20 ORDER BY x LIMIT 7");
            let batches = context.sql(&sql).await.unwrap().collect().await.unwrap();
            let values: Vec<f64> = batches
                .iter()
                .flat_map(|b| {
                    b.column(0)
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .unwrap()
                        .values()
                        .iter()
                        .copied()
                })
                .collect();
            assert_eq!(values, (0..7).map(f64::from).collect::<Vec<_>>());
            results.push(batches);
        }
        assert_eq!(results[0], results[1]);
    }
    assert!(
        requests.load(Ordering::Relaxed) > 0,
        "must exercise the real HTTP adapter"
    );
}

#[tokio::test]
async fn sql_reads_enforce_the_output_budget() {
    let dir = uniformat_testkit::tempdir().unwrap();
    let file = dir.path().join("table.sas7bdat");
    std::fs::write(&file, fixtures::table(100, Compression::None)).unwrap();
    let provider = SasTableProvider::try_new_local(
        &file,
        ReadOptions {
            max_batch_bytes: 512,
            ..ReadOptions::default()
        },
    )
    .await
    .unwrap();
    let context = SessionContext::new();
    context
        .register_table("bounded", Arc::new(provider))
        .unwrap();
    let error = context
        .sql("SELECT * FROM bounded")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("budget mémoire"), "{error}");
}

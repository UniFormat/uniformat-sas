//! Mandatory aggregate-allocation regressions for local and Flight readers.
#[path = "../../../tests/support/fixtures.rs"]
mod fixtures;
use std::sync::Arc;
use uniformat_sas::Compression;
use uniformat_sas_arrow::{BatchReader, Error, ReadOptions};
use uniformat_sas_io::{OpenOptions, SasFile};

#[test]
fn oversized_projection_is_rejected_before_builders_are_allocated() {
    let file = Arc::new(SasFile::open(fixtures::numeric(1), OpenOptions::default()).unwrap());
    let pages = 0..file.meta().header.page_count;
    let opts = ReadOptions {
        max_output_columns: 32,
        ..ReadOptions::default()
    };
    assert!(matches!(
        BatchReader::new(file, pages, Some(vec![0; 64]), None, opts),
        Err(Error::ResourceLimit(_))
    ));
}

#[test]
fn exaggerated_metadata_and_batch_size_cannot_force_large_preallocation() {
    let mut bytes = fixtures::numeric(1);
    fixtures::set_declared_rows(&mut bytes, 1, u64::MAX);
    let file = Arc::new(SasFile::open(bytes, OpenOptions::default()).unwrap());
    let pages = 0..file.meta().header.page_count;
    let opts = ReadOptions {
        batch_rows: usize::MAX,
        max_batch_bytes: 4096,
        ..ReadOptions::default()
    };
    let batches: Vec<_> = BatchReader::new(file, pages, None, Some(1), opts)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 1);
    assert!(batches[0].get_array_memory_size() < 4096);
}

#[test]
fn a_page_exceeding_the_output_budget_returns_one_error_and_no_partial_batch() {
    let file = Arc::new(SasFile::open(fixtures::numeric(1000), OpenOptions::default()).unwrap());
    let pages = 0..file.meta().header.page_count;
    let opts = ReadOptions {
        batch_rows: 1,
        max_batch_bytes: 1024,
        ..ReadOptions::default()
    };
    let mut reader = BatchReader::new(file, pages, None, None, opts).unwrap();
    assert!(matches!(reader.next(), Some(Err(Error::ResourceLimit(_)))));
    assert!(reader.next().is_none());
    assert!(reader.next().is_none());
}

#[test]
fn utf8_width_counts_toward_the_budget_before_decoding() {
    let file = Arc::new(
        SasFile::open(
            fixtures::table(1, Compression::None),
            OpenOptions::default(),
        )
        .unwrap(),
    );
    let pages = 0..file.meta().header.page_count;
    let opts = ReadOptions {
        max_batch_bytes: 520,
        ..ReadOptions::default()
    };
    assert!(matches!(
        BatchReader::new(file, pages, None, None, opts),
        Err(Error::ResourceLimit(_))
    ));
}

#[test]
fn synthetic_reads_keep_every_row_for_all_compressions_and_small_batches() {
    for compression in [Compression::None, Compression::Char, Compression::Binary] {
        let file = Arc::new(
            SasFile::open(fixtures::table(1000, compression), OpenOptions::default()).unwrap(),
        );
        let pages = 0..file.meta().header.page_count;
        let opts = ReadOptions {
            batch_rows: 20,
            ..ReadOptions::default()
        };
        let batches = BatchReader::new(file, pages, None, None, opts)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 1000);
        assert!(batches.iter().all(|b| b.num_columns() == 2));
    }
}

#[cfg(feature = "object-store")]
#[tokio::test]
async fn remote_metadata_prefix_is_capped_before_growth() {
    let dir = uniformat_testkit::tempdir().unwrap();
    std::fs::write(dir.path().join("table.sas7bdat"), fixtures::numeric(1)).unwrap();
    let store =
        Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let opts = ReadOptions {
        max_metadata_bytes: 1024,
        ..ReadOptions::default()
    };
    let error = uniformat_sas_arrow::ObjectFile::open(
        store,
        object_store::path::Path::from("table.sas7bdat"),
        opts,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, Error::ResourceLimit(_)));
}

#[cfg(feature = "object-store")]
#[tokio::test]
async fn remote_stream_yields_before_decoding_the_rest_of_an_input_group() {
    use futures::StreamExt;
    let dir = uniformat_testkit::tempdir().unwrap();
    let mut bytes = fixtures::numeric(50_000);
    let file = SasFile::open(bytes.clone(), OpenOptions::default()).unwrap();
    let h = &file.meta().header;
    let last = h.header_length as usize + (h.page_count as usize - 1) * h.page_length as usize;
    // Corrupt the block count of the last data page, after several valid output batches.
    assert_eq!(
        u16::from_le_bytes(bytes[last + 32..last + 34].try_into().unwrap()) & 0x0f00,
        0x0100
    );
    bytes[last + 34..last + 36].copy_from_slice(&u16::MAX.to_le_bytes());
    std::fs::write(dir.path().join("table.sas7bdat"), bytes).unwrap();
    let store =
        Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let obj = uniformat_sas_arrow::ObjectFile::open(
        store,
        object_store::path::Path::from("table.sas7bdat"),
        ReadOptions {
            batch_rows: 1,
            group_bytes: 8 * 1024 * 1024,
            ..ReadOptions::default()
        },
    )
    .await
    .unwrap();
    let mut stream = obj.stream_all(None, None).unwrap();
    assert!(stream.next().await.unwrap().unwrap().num_rows() > 0);
    let mut rejected = false;
    while let Some(batch) = stream.next().await {
        if batch.is_err() {
            rejected = true;
            break;
        }
    }
    assert!(rejected, "the later malformed page must still be validated");
    assert!(stream.next().await.is_none());
}

#[cfg(feature = "object-store")]
#[tokio::test]
async fn remote_full_plan_is_bounded_and_small_limits_do_not_expand_all_ranges() {
    use futures::StreamExt;
    use std::io::Write;
    let dir = uniformat_testkit::tempdir().unwrap();
    let mut bytes = fixtures::numeric(10_000);
    let parsed = SasFile::open(bytes.clone(), OpenOptions::default()).unwrap();
    let h = &parsed.meta().header;
    let page_count = 100_000u64;
    let size = u64::from(h.header_length) + page_count * u64::from(h.page_length);
    // Sparse generated file: only the small valid prefix occupies disk blocks.
    bytes[208..216].copy_from_slice(&page_count.to_le_bytes());
    let mut file = std::fs::File::create(dir.path().join("sparse.sas7bdat")).unwrap();
    file.write_all(&bytes).unwrap();
    file.set_len(size).unwrap();
    drop(file);
    let store =
        Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let obj = uniformat_sas_arrow::ObjectFile::open(
        store,
        object_store::path::Path::from("sparse.sas7bdat"),
        ReadOptions {
            plan: uniformat_sas_io::PlanMode::Full,
            max_metadata_bytes: 256 * 1024,
            group_bytes: 1,
            ..ReadOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        obj.page_plan().await,
        Err(Error::ResourceLimit(_))
    ));
    let mut stream = obj.stream_all(None, Some(1)).unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap().num_rows(), 1);
    assert!(stream.next().await.is_none());
    let invalid = uniformat_sas_io::PageGroup {
        pages: 0..page_count + 1,
        rows_before: 0,
        rows: 0,
    };
    assert!(obj.stream(&invalid, None, None, Some(1)).is_err());
}

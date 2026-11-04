//! The SAS format through the `uniformat-core` interface (registry, info, read, write, convert).

use std::path::Path;

use uniformat_core::{convert, Error, ReadRequest, Registry, WriteRequest};
use uniformat_sas_arrow::register;
use uniformat_testkit::Corpus;

fn registry() -> Registry {
    let mut r = Registry::new();
    register(&mut r);
    r
}

fn req(pairs: &[(&str, &str)]) -> ReadRequest {
    ReadRequest {
        options: pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        ..ReadRequest::default()
    }
}

#[test]
fn registered_by_name_and_extension() {
    let r = registry();
    assert_eq!(r.by_name("SAS7BDAT").unwrap().name(), "sas7bdat");
    assert_eq!(
        r.by_path(Path::new("dir/T.SAS7BDAT")).unwrap().name(),
        "sas7bdat"
    );
    assert!(r.by_name("sas7bdat").unwrap().can_write());
}

#[test]
fn options_are_validated() {
    let r = registry();
    let f = r.by_name("sas7bdat").unwrap();
    let err = f
        .read(Path::new("absent.sas7bdat"), &req(&[("bogus", "1")]))
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(err, Error::InvalidOption { .. }), "{err}");
    let err = f
        .read(Path::new("absent.sas7bdat"), &req(&[("missing", "maybe")]))
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(err, Error::InvalidOption { .. }), "{err}");
    let err = f
        .read(Path::new("absent.sas7bdat"), &ReadRequest::default())
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(err, Error::Io(_)), "{err}");
}

#[test]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
fn info_read_and_convert_on_the_corpus() {
    let corpus = Corpus::require_from_env();
    let r = registry();
    let f = r.by_name("sas7bdat").unwrap();
    let src = corpus
        .root()
        .join("files/valeurs/valeurs_manquants_no.sas7bdat");
    let info = f.info(&src, &req(&[("missing", "sidecar")])).unwrap();
    assert_eq!(info.row_count, Some(28));
    assert!(info.schema.field_with_name("x__missing").is_ok());
    // read with a projection and a limit
    let mut rq = req(&[("missing", "sidecar")]);
    rq.columns = Some(vec!["x__missing".into()]);
    rq.limit = Some(5);
    let rows: usize = f
        .read(&src, &rq)
        .unwrap()
        .map(|b| b.unwrap().num_rows())
        .sum();
    assert_eq!(rows, 5);
    // sas -> sas through the common model, plain to BINARY compressed, then compare
    let dir = std::env::temp_dir().join(format!("uniformat-sas-format-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dst = dir.join("copy.sas7bdat");
    let write = WriteRequest {
        options: [("compression".to_owned(), "binary".to_owned())].into(),
    };
    let summary = convert(
        &r,
        &src,
        &dst,
        None,
        None,
        &req(&[("missing", "sidecar")]),
        &write,
    )
    .unwrap();
    assert_eq!(summary.rows, 28);
    let codes: Vec<u8> = f
        .read(&dst, &req(&[("missing", "sidecar")]))
        .unwrap()
        .flat_map(|b| {
            let b = b.unwrap();
            let a = b
                .column_by_name("x__missing")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow_array::UInt8Array>()
                .unwrap()
                .clone();
            a.iter().flatten().collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(codes, (1u8..=28).collect::<Vec<_>>());
    // an invalid write leaves no file
    let bad = WriteRequest {
        options: [("compression".to_owned(), "zip".to_owned())].into(),
    };
    let dst2 = dir.join("never.sas7bdat");
    assert!(convert(&r, &src, &dst2, None, None, &ReadRequest::default(), &bad).is_err());
    assert!(!dst2.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

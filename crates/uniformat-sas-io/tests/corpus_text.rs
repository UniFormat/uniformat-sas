//! Critère de fin de J3 (4) : les octets invalides de `enc_inv` (`30` §6.2) se relisent octet pour octet en mode
//! `Binary`, sont comptés en mode `Replace` et refusés en mode `Error`. Sauté sans `UNIFORMAT_CORPUS`.

use uniformat_sas::{ColumnKind, Error, InvalidBytes, TextPolicy, TextValue};
use uniformat_sas_io::{FileSource, OpenOptions, SasFile};
use uniformat_testkit::Corpus;

const EXPECTED: [&[u8]; 3] = [b"ab\xffdef", b"ab\x80d\xc3\xa9", b"abcd\xe6\x97"];

#[test]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
fn invalid_bytes_policies() {
    let corpus = Corpus::require_from_env();
    let path = corpus.root().join("gen/probe/enc/files/enc_inv.sas7bdat");
    let file = SasFile::open(
        FileSource::open(&path).expect("ouverture"),
        OpenOptions::default(),
    )
    .expect("open");
    let meta = file.meta();
    let text_cols: Vec<_> = meta
        .columns
        .iter()
        .filter(|c| matches!(c.kind, ColumnKind::Char { .. }))
        .collect();
    assert!(!text_cols.is_empty());

    let (mut binary, mut replaced, mut errors, mut valid) = (Vec::new(), 0usize, 0usize, 0usize);
    file.for_each_row(|row| {
        for c in &text_cols {
            match row
                .text_value(
                    c,
                    &TextPolicy {
                        invalid: InvalidBytes::Binary,
                        trim_trailing: true,
                    },
                )
                .unwrap()
            {
                TextValue::Bytes(b) => binary.push(b.to_vec()),
                TextValue::Text { .. } => valid += 1,
            }
            if let TextValue::Text {
                replaced: true,
                value,
            } = row
                .text_value(
                    c,
                    &TextPolicy {
                        invalid: InvalidBytes::Replace,
                        trim_trailing: true,
                    },
                )
                .unwrap()
            {
                assert!(value.contains('\u{FFFD}'));
                replaced += 1;
            }
            let strict = TextPolicy {
                invalid: InvalidBytes::Error,
                trim_trailing: true,
            };
            if matches!(row.text_value(c, &strict), Err(Error::Encoding { .. })) {
                errors += 1;
            }
        }
    })
    .expect("lecture");
    let expected: Vec<Vec<u8>> = EXPECTED.iter().map(|b| b.to_vec()).collect();
    assert_eq!(binary, expected, "octets relus en mode Binary");
    assert_eq!((replaced, errors), (3, 3), "compteurs Replace et Error");
    eprintln!("enc_inv : 3 champs invalides relus octet pour octet, {valid} champs valides");
}

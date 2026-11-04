//! Critère de fin de J3 (1) : la table de classification attribue un type logique à **chaque nom** des tables de
//! `30` §1 (241 couples) et §1.1 (388 couples) ; la table des encodages couvre les 127 codes de `30` §6.1.
//! Les tests relisent la spécification Markdown : une table générée périmée fait échouer le test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use uniformat_sas::{
    classify_format, encoding_for_code, encoding_info, is_native_format, LogicalType, StorageKind,
};

fn spec() -> Option<String> {
    let p =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/30_formats_encodages.md");
    std::fs::read_to_string(p).ok()
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_owned())
        .collect()
}

/// Type logique attendu pour un stockage numérique, déduit de la catégorie en français de la spécification.
fn expected_numeric(category: &str) -> LogicalType {
    let c = category.to_lowercase();
    if c.starts_with("datetime") {
        LogicalType::DateTime
    } else if c.starts_with("date") {
        LogicalType::Date
    } else if c.starts_with("heure") {
        LogicalType::Time
    } else {
        LogicalType::Number
    }
}

#[test]
fn every_listed_name_is_classified() {
    let Some(text) = spec() else {
        eprintln!("spécification absente : test sauté");
        return;
    };
    let (mut section, mut n1, mut n11) = (0, 0usize, 0usize);
    // (nom, sens) -> catégorie
    let mut expected: BTreeMap<(String, String), String> = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with("## 1. Table") {
            section = 1;
        } else if line.starts_with("### 1.1") {
            section = 11;
        } else if line.starts_with("## 2.") {
            section = 0;
        }
        if section != 0 && line.starts_with("| `") {
            let c = cells(line);
            let name = c[0].trim_matches('`').to_uppercase();
            let category = if section == 1 { &c[2] } else { &c[3] };
            if section == 1 {
                n1 += 1;
            } else {
                n11 += 1;
            }
            expected.insert((name, c[1].clone()), category.clone());
        }
    }
    assert_eq!(
        (n1, n11),
        (241, 388),
        "nombre de couples nom/sens de 30 §1 et §1.1"
    );
    for ((name, sense), category) in &expected {
        assert!(
            is_native_format(name),
            "{name} ({sense}) absent de la table"
        );
        // classify_format regarde d'abord le sens « format », puis « informat »
        let wanted =
            if sense == "format" || !expected.contains_key(&(name.clone(), "format".to_owned())) {
                Some(expected_numeric(category))
            } else {
                None
            };
        if let Some(wanted) = wanted {
            assert_eq!(
                classify_format(name, StorageKind::Numeric),
                wanted,
                "{name} ({sense}, {category})"
            );
        }
        assert_eq!(
            classify_format(name, StorageKind::Char),
            LogicalType::Text,
            "{name} sur une colonne caractère"
        );
    }
}

#[test]
fn all_127_encoding_codes_of_the_spec_are_in_the_table() {
    let Some(text) = spec() else {
        eprintln!("spécification absente : test sauté");
        return;
    };
    let mut seen = 0;
    for line in text.lines() {
        let Some(first) = line.strip_prefix("| ").and_then(|l| l.split(' ').next()) else {
            continue;
        };
        let Ok(code) = first.parse::<u8>() else {
            continue;
        };
        if !line.contains("| inféré") && !line.contains("confirmé") {
            continue;
        }
        let c = cells(line);
        if c.len() < 8 {
            continue;
        }
        seen += 1;
        assert!(encoding_info(code).is_some(), "code {code} absent");
        let has_standard = c[5].starts_with('`');
        assert_eq!(
            encoding_for_code(code).is_some(),
            has_standard,
            "code {code} : {}",
            c[5]
        );
    }
    assert_eq!(seen, 127);
}

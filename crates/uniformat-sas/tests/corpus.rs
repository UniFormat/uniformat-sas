//! Inventaire du corpus : seul test d'intégration du jalon J0 (la lecture arrive au jalon J1).
//! Sauté sans `UNIFORMAT_CORPUS` (ADR 0007 : les fichiers du SAS complet ne sont pas redistribués).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use uniformat_testkit::{Corpus, ReferenceTable};

#[test]
fn corpus_inventory_matches_manifest() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let files = corpus.files().expect("lecture du corpus");
    // 232 fichiers analysés par 10 §9 : files/ (SAS complet) + external/ (fichiers publics).
    assert_eq!(
        files.len(),
        232,
        "inventaire inattendu : voir corpus/MANIFEST.md"
    );
    for f in files.iter().filter(|f| f.csv.is_some()) {
        let bytes = std::fs::read(f.csv.as_ref().unwrap()).expect("CSV lisible");
        ReferenceTable::parse_bytes(&bytes)
            .unwrap_or_else(|e| panic!("{} : {e}", f.path.display()));
    }
}

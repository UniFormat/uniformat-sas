//! Critère de fin de J3 (5) : **0 avertissement inattendu** sur les 232 fichiers analysés. La liste des avertissements
//! attendus est versionnée dans `expected_warnings.tsv` (fichier, identifiants séparés par des virgules) ; toute
//! différence échoue. `SAS7BDAT_UPDATE_EXPECTED=1` réécrit la liste (à relire avant de la valider).
//! Sauté sans `UNIFORMAT_CORPUS`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use uniformat_sas_io::{FileSource, OpenOptions, SasFile};
use uniformat_testkit::Corpus;

fn expected_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/expected_warnings.tsv")
}

#[test]
fn no_unexpected_warnings() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let files = corpus.files().expect("lecture du corpus");
    let mut actual: BTreeMap<String, String> = BTreeMap::new();
    for f in &files {
        let rel = f
            .path
            .strip_prefix(corpus.root())
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let file = SasFile::open(FileSource::open(&f.path).unwrap(), OpenOptions::default())
            .unwrap_or_else(|e| panic!("{rel} : {e}"));
        let mut ids: Vec<&'static str> = file.report().warnings().iter().map(|w| w.id).collect();
        let decode = file
            .for_each_row(|_| {})
            .unwrap_or_else(|e| panic!("{rel} : {e}"));
        ids.extend(decode.warnings().iter().map(|w| w.id));
        ids.sort_unstable();
        ids.dedup();
        if !ids.is_empty() {
            actual.insert(rel, ids.join(","));
        }
    }
    let render = |m: &BTreeMap<String, String>| {
        m.iter()
            .map(|(k, v)| format!("{k}\t{v}\n"))
            .collect::<String>()
    };
    if std::env::var_os("SAS7BDAT_UPDATE_EXPECTED").is_some() {
        std::fs::write(expected_path(), render(&actual)).unwrap();
        eprintln!(
            "{} fichiers avec avertissements : liste réécrite",
            actual.len()
        );
        return;
    }
    let expected: BTreeMap<String, String> = std::fs::read_to_string(expected_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let unexpected: Vec<String> = actual
        .iter()
        .filter(|(k, v)| expected.get(*k) != Some(v))
        .map(|(k, v)| format!("{k} : {v} (attendu {:?})", expected.get(k)))
        .chain(
            expected
                .keys()
                .filter(|k| !actual.contains_key(*k))
                .map(|k| format!("{k} : avertissement attendu absent")),
        )
        .collect();
    assert!(
        unexpected.is_empty(),
        "{} écarts :\n{}",
        unexpected.len(),
        unexpected.join("\n")
    );
    eprintln!(
        "{} fichiers avec avertissements attendus, 0 inattendu sur {}",
        expected.len(),
        files.len()
    );
}

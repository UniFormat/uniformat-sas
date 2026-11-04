//! Critères de fin de J1 et J2 (`docs/roadmap.md`) : lecture des tables, compressées ou non, du corpus sans écart de valeur
//! par rapport aux CSV de référence, comparaison par nom de colonne. Sauté sans `UNIFORMAT_CORPUS`.

use uniformat_sas::{ColumnKind, Compression, MissingKind, NumValue};
use uniformat_sas_io::{FileSource, OpenOptions, SasFile};
use uniformat_testkit::{values_equal, Corpus, ReferenceTable};

fn missing_text(m: MissingKind) -> String {
    match m {
        MissingKind::Dot => ".".to_owned(),
        MissingKind::Underscore => "_".to_owned(),
        MissingKind::Letter(c) => char::from(c).to_string(),
    }
}

/// Écarts d'un fichier (vide si aucun).
fn compare(
    path: &std::path::Path,
    csv: &std::path::Path,
    external: bool,
) -> Result<(Vec<String>, bool, bool), String> {
    let source = FileSource::open(path).map_err(|e| format!("ouverture : {e}"))?;
    // U22 (10 §5.4) : datareader/test16 et test19 portent le code 29 ou 62 mais contiennent de l'UTF-8.
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let mut options = OpenOptions::default();
    // jeu de données B de datareader : test16 à test21
    let dataset_b = stem
        .strip_prefix("test")
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| (16..=21).contains(&n));
    if external && path.to_string_lossy().contains("datareader") && dataset_b {
        options.encoding_override = Some(uniformat_sas::UTF_8);
    }
    let file = SasFile::open(source, options).map_err(|e| format!("open : {e}"))?;
    let meta = file.meta();
    let compressed = meta.compression != Compression::None;
    let reference = ReferenceTable::parse_bytes(&std::fs::read(csv).map_err(|e| e.to_string())?)
        .map_err(|e| format!("csv : {e}"))?;
    let mut diffs = Vec::new();
    for c in &meta.columns {
        if !reference.columns().contains(&c.name) {
            diffs.push(format!("colonne {:?} absente du CSV", c.name));
        }
    }
    if meta.columns.len() != reference.columns().len() {
        diffs.push(format!(
            "{} colonnes contre {} dans le CSV",
            meta.columns.len(),
            reference.columns().len()
        ));
    }
    let mut n = 0usize;
    file.for_each_row(|row| {
        let i = n;
        n += 1;
        if diffs.len() >= 5 {
            return;
        }
        for c in &meta.columns {
            let Some(expected) = reference.cell(i, &c.name) else {
                continue;
            };
            let ok = match c.kind {
                ColumnKind::Numeric { .. } => match row.num(c) {
                    NumValue::Value(v) => values_equal(expected, &format!("{v:?}")),
                    NumValue::Missing(m) => {
                        expected == missing_text(m) || (external && expected.is_empty())
                    }
                },
                // Octets invalides pour l'encodage : le CSV, relu en Latin-1 octet par octet, est comparé aux octets
                // (la politique de décodage fait l'objet de J3).
                ColumnKind::Char { .. } => {
                    row.text(c) == expected
                        || (expected.chars().all(|ch| u32::from(ch) < 256)
                            && expected
                                .chars()
                                .map(|ch| u8::try_from(u32::from(ch)).unwrap_or(0))
                                .eq(row.char_bytes(c).iter().copied()))
                }
            };
            if !ok {
                diffs.push(format!("ligne {i} colonne {:?} : CSV {expected:?}", c.name));
                break;
            }
        }
    })
    .map_err(|e| format!("lecture : {e}"))?;
    if n as u64 != meta.visible_rows() {
        diffs.push(format!(
            "{n} lignes lues contre {} visibles (RowSize)",
            meta.visible_rows()
        ));
    }
    if n != reference.row_count() {
        diffs.push(format!(
            "{n} lignes lues contre {} dans le CSV",
            reference.row_count()
        ));
    }
    Ok((diffs, compressed, meta.header.encoding_code != 20))
}

#[test]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
fn all_tables_with_reference_csv_match() {
    let corpus = Corpus::require_from_env();
    let files = corpus.files().expect("lecture du corpus");
    let (mut read, mut compressed, mut no_csv, mut non_utf8_files) =
        (0usize, 0usize, 0usize, 0usize);
    let mut failures = Vec::new();
    for f in &files {
        let Some(csv) = &f.csv else {
            no_csv += 1;
            continue;
        };
        match compare(
            &f.path,
            csv,
            f.family == "datareader" || f.family == "pandas" || f.family == "pyreadstat",
        ) {
            Ok((diffs, is_compressed, non_utf8)) => {
                non_utf8_files += usize::from(non_utf8);
                if is_compressed {
                    compressed += 1;
                } else {
                    read += 1;
                }
                if !diffs.is_empty() {
                    failures.push(format!(
                        "{} :\n    {}",
                        f.path.display(),
                        diffs.join("\n    ")
                    ));
                }
            }
            Err(e) => failures.push(format!("{} : {e}", f.path.display())),
        }
    }
    eprintln!("{read} fichiers non compressés et {compressed} compressés lus, {no_csv} sans CSV");
    eprintln!("{non_utf8_files} fichiers dont le code d'encodage n'est pas 20 (UTF-8)");
    assert_eq!(
        non_utf8_files, 50,
        "J3 : 50 fichiers non UTF-8 (latin1, wlatin1, Shift-JIS)"
    );
    assert_eq!(
        (read, compressed, no_csv),
        (173, 45, 14),
        "inventaire inattendu"
    );
    assert!(
        failures.is_empty(),
        "{} fichiers en écart :\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
fn deleted_rows_are_invisible() {
    let corpus = Corpus::require_from_env();
    let path = corpus
        .root()
        .join("files/suppression/suppression_n100_del10_no.sas7bdat");
    let file = SasFile::open(
        FileSource::open(&path).expect("ouverture"),
        OpenOptions::default(),
    )
    .expect("open");
    let mut n = 0u64;
    file.for_each_row(|_| n += 1).expect("lecture");
    assert_eq!(
        (file.meta().row_count, file.meta().deleted_count, n),
        (100, 10, 90)
    );
}

/// Les fichiers sans CSV (publics compressés notamment) se lisent sans erreur et rendent `row_count - deleted` lignes.
#[test]
#[ignore = "external SAS corpus: set UNIFORMAT_CORPUS and run with --ignored"]
fn every_file_reads_and_row_counts_match() {
    let corpus = Corpus::require_from_env();
    let files = corpus.files().expect("lecture du corpus");
    let mut failures = Vec::new();
    let (mut compressed, mut rows_total) = (0usize, 0u64);
    for f in &files {
        let mut options = OpenOptions::default();
        // jeu de données B de datareader (U22) : voir `compare`
        let stem = f.stem();
        if f.path.to_string_lossy().contains("datareader")
            && stem
                .strip_prefix("test")
                .and_then(|n| n.parse::<u32>().ok())
                .is_some_and(|n| n >= 16)
        {
            options.encoding_override = Some(uniformat_sas::UTF_8);
        }
        let result = FileSource::open(&f.path)
            .map_err(|e| e.to_string())
            .and_then(|s| SasFile::open(s, options).map_err(|e| format!("open : {e}")))
            .and_then(|file| {
                let mut n = 0u64;
                file.for_each_row(|_| n += 1)
                    .map_err(|e| format!("lecture : {e}"))?;
                Ok((
                    n,
                    file.meta().visible_rows(),
                    file.meta().compression != Compression::None,
                ))
            });
        match result {
            Ok((n, expected, is_compressed)) => {
                compressed += usize::from(is_compressed);
                rows_total += n;
                if n != expected {
                    failures.push(format!(
                        "{} : {n} lignes lues, {expected} attendues",
                        f.path.display()
                    ));
                }
            }
            Err(e) => failures.push(format!("{} : {e}", f.path.display())),
        }
    }
    eprintln!(
        "{} fichiers, dont {compressed} compressés, {rows_total} lignes lues",
        files.len()
    );
    assert!(
        failures.is_empty(),
        "{} fichiers en écart :\n{}",
        failures.len(),
        failures.join("\n")
    );
}

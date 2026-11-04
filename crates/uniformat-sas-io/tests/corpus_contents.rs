//! Critère de fin de J1 (2) : les métadonnées concordent avec les 195 sorties `PROC CONTENTS` du corpus
//! (nombre d'observations et de variables, observations supprimées, longueur de ligne, compression, taille et
//! nombre de pages, release ; par variable : rang, nom, type, longueur, format, informat, label).
//! Sauté sans `UNIFORMAT_CORPUS`.

use std::collections::HashMap;

use uniformat_sas::{ColumnKind, Compression, SasFormat};
use uniformat_sas_io::{FileSource, OpenOptions, SasFile};
use uniformat_testkit::Corpus;

#[derive(Debug)]
struct Var {
    rank: usize,
    numeric: bool,
    len: u64,
    format: String,
    informat: String,
    label: String,
}

/// Valeur qui suit `key` sur une ligne de l'en-tête de PROC CONTENTS (deux blocs côte à côte par ligne).
fn header_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let mut from = 0;
        while let Some(rel) = line[from..].find(key) {
            let at = from + rel;
            let before_ok = line[..at].chars().next_back().is_none_or(|c| c == ' ');
            let after = &line[at + key.len()..];
            // « Observations » ne doit pas être le second mot de « Deleted Observations ».
            let deleted = key == "Observations" && line[..at].trim_end().ends_with("Deleted");
            if before_ok && !deleted && after.starts_with(' ') {
                if let Some(v) = after.split_whitespace().next() {
                    return Some(v.to_owned());
                }
            }
            from = at + key.len();
        }
    }
    None
}

fn parse_variables(text: &str) -> Option<HashMap<String, Var>> {
    let seg = text
        .split("Alphabetic List of Variables and Attributes")
        .last()?;
    let lines: Vec<Vec<char>> = seg.lines().map(|l| l.chars().collect()).collect();
    let head_idx = lines.iter().position(|l| {
        let s: String = l.iter().collect();
        s.contains("Variable") && s.contains("Type") && s.contains("Len")
    })?;
    let head: String = lines[head_idx].iter().collect();
    let pos = |k: &str| head.find(k).map(|b| head[..b].chars().count());
    let keys = ["Variable", "Type", "Len", "Format", "Informat", "Label"];
    let cols: Vec<(&str, usize)> = keys
        .iter()
        .filter_map(|k| pos(k).map(|p| (*k, p)))
        .collect();
    let mut out = HashMap::new();
    for l in &lines[head_idx + 1..] {
        let s: String = l.iter().collect();
        let mut it = s.split_whitespace();
        let (Some(rank), Some(name), Some(ty), Some(len)) =
            (it.next(), it.next(), it.next(), it.next())
        else {
            continue;
        };
        let (Ok(rank), true, Ok(len)) = (
            rank.parse::<usize>(),
            ty == "Num" || ty == "Char",
            len.parse::<u64>(),
        ) else {
            continue;
        };
        // SAS remplit la colonne Variable en octets : les colonnes suivantes reculent des caractères multi-octets.
        let extra = name.len() - name.chars().count();
        let cut = |k: &str| -> String {
            let Some(&(_, start)) = cols.iter().find(|(n, _)| *n == k) else {
                return String::new();
            };
            let start = start.saturating_sub(extra);
            let next = cols
                .iter()
                .map(|&(_, p)| p.saturating_sub(extra))
                .filter(|&p| p > start)
                .min();
            let end = next.unwrap_or(l.len()).min(l.len());
            if start >= end {
                return String::new();
            }
            l[start..end].iter().collect::<String>().trim().to_owned()
        };
        out.insert(
            name.to_owned(),
            Var {
                rank,
                numeric: ty == "Num",
                len,
                format: cut("Format"),
                informat: cut("Informat"),
                label: cut("Label"),
            },
        );
    }
    Some(out)
}

fn fmt_str(f: &Option<SasFormat>) -> String {
    match f {
        Some(f) if !f.name.is_empty() => format!(
            "{}{}.{}",
            f.name,
            if f.width != 0 {
                f.width.to_string()
            } else {
                String::new()
            },
            if f.decimals != 0 {
                f.decimals.to_string()
            } else {
                String::new()
            }
        ),
        _ => String::new(),
    }
}

fn check(path: &std::path::Path, contents: &std::path::Path) -> Result<Vec<String>, String> {
    let file = SasFile::open(
        FileSource::open(path).map_err(|e| e.to_string())?,
        OpenOptions::default(),
    )
    .map_err(|e| format!("open : {e}"))?;
    let meta = file.meta();
    let text = std::fs::read_to_string(contents).map_err(|e| e.to_string())?;
    let mut errs = Vec::new();
    let mut want = |key: &str, got: String| match header_value(&text, key) {
        Some(w) if w == got => {}
        other => errs.push(format!("{key} : contents={other:?} lecteur={got:?}")),
    };
    want("Observations", meta.visible_rows().to_string());
    want("Deleted Observations", meta.deleted_count.to_string()); // « Deleted Observations 10 » : la valeur suit « Observations »
    want("Variables", meta.columns.len().to_string());
    want("Observation Length", meta.row_length.to_string());
    want(
        "Compressed",
        match meta.compression {
            Compression::None => "NO",
            Compression::Char => "CHAR",
            Compression::Binary => "BINARY",
        }
        .to_owned(),
    );
    want("Data Set Page Size", meta.header.page_length.to_string());
    Ok(errs)
}

#[test]
fn metadata_matches_proc_contents() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let files = corpus.files().expect("lecture du corpus");
    let (mut total, mut bad) = (0usize, Vec::new());
    for f in files.iter().filter(|f| f.contents.is_some()) {
        total += 1;
        let contents = f.contents.as_ref().unwrap();
        let file = SasFile::open(FileSource::open(&f.path).unwrap(), OpenOptions::default());
        let file = match file {
            Ok(file) => file,
            Err(e) => {
                bad.push(format!("{} : open : {e}", f.stem()));
                continue;
            }
        };
        let meta = file.meta();
        let text = std::fs::read_to_string(contents).unwrap_or_default();
        let Some(vars) = parse_variables(&text) else {
            bad.push(format!("{} : section des variables illisible", f.stem()));
            continue;
        };
        let mut errs = Vec::new();
        if vars.len() != meta.columns.len() {
            errs.push(format!(
                "{} variables contre {}",
                vars.len(),
                meta.columns.len()
            ));
        }
        for (j, c) in meta.columns.iter().enumerate() {
            let Some(v) = vars.get(&c.name) else {
                errs.push(format!("variable {:?} absente de PROC CONTENTS", c.name));
                continue;
            };
            let (numeric, len) = match c.kind {
                ColumnKind::Numeric { length } => (true, u64::from(length)),
                ColumnKind::Char { length } => (false, u64::from(length)),
            };
            let label = c.label.clone().unwrap_or_default();
            // Le listing (linesize=200) tronque les labels très longs.
            let label_ok =
                v.label == label || (label.starts_with(&v.label) && v.label.chars().count() >= 60);
            if v.rank != j + 1 || v.numeric != numeric || v.len != len || !label_ok {
                errs.push(format!("{} : rang/type/longueur/label ({v:?})", c.name));
            }
            for (what, want, got) in [
                ("format", &v.format, fmt_str(&c.format)),
                ("informat", &v.informat, fmt_str(&c.informat)),
            ] {
                if *want != got {
                    errs.push(format!(
                        "{}.{what} : contents={want:?} lecteur={got:?}",
                        c.name
                    ));
                }
            }
        }
        match check(&f.path, contents) {
            Ok(mut e) => errs.append(&mut e),
            Err(e) => errs.push(e),
        }
        if !errs.is_empty() {
            bad.push(format!("{} : {}", f.stem(), errs.join(" ; ")));
        }
    }
    eprintln!("{total} PROC CONTENTS comparés, {} en écart", bad.len());
    assert_eq!(total, 195, "nombre de PROC CONTENTS inattendu");
    assert!(
        bad.is_empty(),
        "{} écarts :\n{}",
        bad.len(),
        bad.iter().take(25).cloned().collect::<Vec<_>>().join("\n")
    );
}

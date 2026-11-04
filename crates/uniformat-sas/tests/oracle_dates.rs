//! Critère de fin de J3 (3) : les conversions de dates, date-heures et heures retrouvent les sorties `PUT(valeur,
//! format)` de l'oracle (`tests/data/fmt_display_*.txt`, échantillon de `30` §4.4). Hors plage 1582…20 000,
//! la conversion rapporte `OutOfRange` (SAS écrit des astérisques) : jamais de valeur silencieuse.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use uniformat_sas::temporal::{
    sas_date_to_unix_days, sas_datetime_to_parts, sas_datetime_to_unix_micros, sas_days_to_ymd,
    sas_time_to_micros, ConvertError,
};

/// Lignes `valeur|résultat` d'un format donné (`debut` et valeurs manquantes écartés).
fn cases(file: &str, format: &str) -> Option<Vec<(f64, String)>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data")
        .join(file);
    let text = std::fs::read_to_string(p).ok()?;
    let prefix = format!("{format}|");
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        let (value, result) = rest.split_once('|').unwrap();
        let Ok(v) = value.parse::<f64>() else {
            continue;
        };
        out.push((v, result.trim_end().to_owned()));
    }
    Some(out)
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

#[test]
fn dates_match_yymmdd10() {
    let Some(cases) = cases("fmt_display_date.txt", "yymmdd10.") else {
        eprintln!("évidence absente : test sauté");
        return;
    };
    assert!(cases.len() >= 35, "{} cas", cases.len());
    let (mut shown, mut stars) = (0, 0);
    for (v, expected) in &cases {
        let got = sas_days_to_ymd(*v as i64);
        if expected.chars().all(|c| c == '*') {
            assert_eq!(
                got,
                Err(ConvertError::OutOfRange),
                "{v} : SAS écrit des astérisques"
            );
            assert_eq!(
                sas_date_to_unix_days(*v),
                Err(ConvertError::OutOfRange),
                "{v}"
            );
            stars += 1;
            continue;
        }
        let (y, m, d) = got.unwrap_or_else(|e| panic!("{v} : {e}"));
        // au-delà de l'an 9 999 le format affiche `****` à la place de l'année
        let year = if y > 9999 {
            "****".to_owned()
        } else {
            format!("{y:04}")
        };
        assert_eq!(format!("{year}-{m:02}-{d:02}"), *expected, "jour SAS {v}");
        shown += 1;
    }
    eprintln!("yymmdd10. : {shown} dates identiques, {stars} hors plage rapportées");
    assert!(
        shown >= 25 && stars >= 5,
        "{shown} dates, {stars} hors plage"
    );
}

#[test]
fn unix_days_are_value_minus_3653() {
    assert_eq!(sas_date_to_unix_days(21915.0), Ok(18262)); // 2020-01-01
}

#[test]
fn datetimes_match_datetime19() {
    let Some(cases) = cases("fmt_display_dt.txt", "datetime19.") else {
        eprintln!("évidence absente : test sauté");
        return;
    };
    assert!(cases.len() >= 14);
    for (v, expected) in &cases {
        let p = sas_datetime_to_parts(*v).unwrap_or_else(|e| panic!("{v} : {e}"));
        let got = format!(
            "{:02}{}{:04}:{:02}:{:02}:{:02}",
            p.day,
            MONTHS[usize::from(p.month) - 1],
            p.year,
            p.hour,
            p.minute,
            p.second
        );
        assert_eq!(got, expected.trim_start(), "datetime SAS {v}");
        assert!(sas_datetime_to_unix_micros(*v).is_ok());
    }
    assert_eq!(
        sas_datetime_to_unix_micros(f64::NAN),
        Err(ConvertError::NotFinite)
    );
}

#[test]
fn times_match_time8() {
    let Some(cases) = cases("fmt_display_time.txt", "time8.") else {
        eprintln!("évidence absente : test sauté");
        return;
    };
    assert!(cases.len() >= 5);
    let mut checked = 0;
    for (v, expected) in &cases {
        let micros = sas_time_to_micros(*v).unwrap_or_else(|e| panic!("{v} : {e}"));
        let secs = (micros as f64 / 1e6).round() as i64;
        let (h, m, s) = (secs.abs() / 3600, secs.abs() % 3600 / 60, secs.abs() % 60);
        let got = format!("{}{h}:{m:02}:{s:02}", if secs < 0 { "-" } else { "" });
        assert_eq!(got, expected.trim_start(), "heure SAS {v}");
        checked += 1;
    }
    eprintln!("time8. : {checked} heures identiques (négatives et ≥ 24 h comprises)");
}

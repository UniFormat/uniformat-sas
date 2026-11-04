//! Classification des formats et informats natifs en type logique (`30` §1 et §1.1).
//!
//! Le format ne change jamais le type de stockage (double IEEE ou texte) : il donne un sens à la colonne.
//! La table est générée par `ci/gen_tables.py` à partir de la spécification.

use crate::format_table::FORMAT_TABLE;

/// Sens d'une entrée de la table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sense {
    Format,
    Informat,
}

/// Classe d'une entrée de la table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Number,
    Date,
    DateTime,
    Time,
    Text,
    /// Numérique ou texte selon le type de stockage de la colonne.
    ByStorage,
}

/// Type logique d'une colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalType {
    /// Nombre (double IEEE).
    Number,
    /// Date : jours depuis 1960-01-01.
    Date,
    /// Date-heure : secondes depuis 1960-01-01T00:00:00.
    DateTime,
    /// Heure : secondes depuis minuit (négatives et ≥ 86 400 admises).
    Time,
    /// Texte.
    Text,
}

/// Type de stockage d'une colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    /// Numérique.
    Numeric,
    /// Caractère.
    Char,
}

fn lookup(name: &str, sense: Sense) -> Option<Class> {
    FORMAT_TABLE
        .binary_search_by(|(n, s, _)| n.cmp(&name).then((*s as u8).cmp(&(sense as u8))))
        .ok()
        .and_then(|i| FORMAT_TABLE.get(i))
        .map(|&(_, _, class)| class)
}

/// Vrai si `name` (sans largeur ni point) figure dans la table des formats ou informats natifs.
/// Sinon c'est un format utilisateur, à résoudre dans un catalogue `.sas7bdat`/`.sas7bcat`.
pub fn is_native_format(name: &str) -> bool {
    let upper = name.trim().to_uppercase();
    lookup(&upper, Sense::Format)
        .or_else(|| lookup(&upper, Sense::Informat))
        .is_some()
}

/// Type logique d'une colonne de type de stockage `storage` portant le format (ou informat) `name`.
///
/// Une colonne caractère reste du texte et une colonne numérique reste un nombre quel que soit le format ;
/// un nom hors table (format utilisateur) suit le type de stockage.
pub fn classify_format(name: &str, storage: StorageKind) -> LogicalType {
    let upper = name.trim().to_uppercase();
    let class = lookup(&upper, Sense::Format).or_else(|| lookup(&upper, Sense::Informat));
    match (storage, class) {
        (StorageKind::Char, _) => LogicalType::Text,
        (StorageKind::Numeric, Some(Class::Date)) => LogicalType::Date,
        (StorageKind::Numeric, Some(Class::DateTime)) => LogicalType::DateTime,
        (StorageKind::Numeric, Some(Class::Time)) => LogicalType::Time,
        (StorageKind::Numeric, _) => LogicalType::Number,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        for w in FORMAT_TABLE.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert!((a.0, a.1 as u8) < (b.0, b.1 as u8), "{} {}", a.0, b.0);
        }
    }

    #[test]
    fn classifies_common_names() {
        let n = StorageKind::Numeric;
        assert_eq!(classify_format("date", n), LogicalType::Date);
        assert_eq!(classify_format("DATE", n), LogicalType::Date);
        assert_eq!(classify_format("MMDDYY", n), LogicalType::Date);
        assert_eq!(classify_format("DATETIME", n), LogicalType::DateTime);
        assert_eq!(classify_format("TIME", n), LogicalType::Time);
        assert_eq!(classify_format("TOD", n), LogicalType::DateTime);
        assert_eq!(classify_format("BEST", n), LogicalType::Number);
        assert_eq!(classify_format("COMMA", n), LogicalType::Number);
        assert_eq!(
            classify_format("$CHAR", StorageKind::Char),
            LogicalType::Text
        );
    }

    #[test]
    fn storage_wins_over_format() {
        assert_eq!(
            classify_format("DATE", StorageKind::Char),
            LogicalType::Text
        );
        assert_eq!(
            classify_format("$CHAR", StorageKind::Numeric),
            LogicalType::Number
        );
    }

    #[test]
    fn unknown_name_is_a_user_format() {
        assert!(!is_native_format("MYFMT"));
        assert_eq!(
            classify_format("MYFMT", StorageKind::Numeric),
            LogicalType::Number
        );
        assert_eq!(classify_format("", StorageKind::Char), LogicalType::Text);
        assert!(is_native_format("date"));
    }
}

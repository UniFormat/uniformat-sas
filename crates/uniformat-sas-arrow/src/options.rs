//! Options de lecture Arrow (ADR 0003, ADR 0004).

use uniformat_sas::{Encoding, InvalidBytes, Strictness};
use uniformat_sas_io::{OpenOptions, PlanMode};

/// Blancs finaux des champs caractère (ADR 0003, ADR 0004 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailingBlanks {
    /// Retire les blancs (et NUL) de fin : le comportement de SAS à la lecture (défaut).
    #[default]
    Trim,
    /// Garde les octets du champ, à la largeur SAS.
    Keep,
}

/// Valeurs numériques manquantes (ADR 0003, décision 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MissingMode {
    /// Tout manquant est une valeur nulle ; aucune colonne annexe (défaut).
    #[default]
    Null,
    /// Ajoute `x__missing` (`UInt8`, `0` = présente, `1` = `.`, `2` = `._`, `3` à `28` = `.A` à `.Z`) après chaque
    /// colonne numérique `x` ; `x` est nulle dès que `x__missing ≠ 0`.
    Sidecar,
}

/// Dates, date-heures et heures (ADR 0003, décision 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateMode {
    /// `Date32`, `Timestamp`, `Time64` selon le format (défaut).
    #[default]
    Native,
    /// Valeur SAS brute en `Float64` (jours, secondes) : aucune conversion, aucune plage.
    AsFloat,
}

/// Unité des date-heures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimestampUnit {
    /// Microsecondes (défaut) : conserve les secondes fractionnaires.
    #[default]
    Microsecond,
    /// Secondes, arrondies vers −∞ : l'unité de la table de `30` §1.
    Second,
}

/// Type des heures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeMode {
    /// `Time64(Microsecond)` ; hors de 0…86 400 s, la valeur suit la règle [`OutOfRange`] (défaut).
    #[default]
    Time64,
    /// `Duration(Microsecond)` : représente les heures négatives et supérieures à 24 h.
    Duration,
}

/// Valeur non représentable (date hors 1582…20 000, heure hors 0…86 400 s en `Time64`, datetime hors plage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutOfRange {
    /// Valeur nulle et compteur [`crate::ReadStats::out_of_range`] (défaut).
    #[default]
    Null,
    /// Erreur typée.
    Error,
}

/// Options de lecture.
#[derive(Debug, Clone)]
pub struct ReadOptions {
    /// Blancs finaux des chaînes.
    pub trailing_blanks: TrailingBlanks,
    /// Valeurs manquantes spéciales.
    pub missing: MissingMode,
    /// Suffixe de la colonne annexe (`x` + suffixe), `__missing` par défaut.
    pub missing_suffix: String,
    /// Dates, date-heures, heures.
    pub dates: DateMode,
    /// Unité des date-heures.
    pub timestamp_unit: TimestampUnit,
    /// Type des heures.
    pub time_mode: TimeMode,
    /// Valeur hors plage.
    pub out_of_range: OutOfRange,
    /// Octets invalides pour l'encodage : `Replace` (défaut, U+FFFD et compteur), `Error`, ou `Binary` (toutes les
    /// colonnes caractère deviennent `Binary`, octets du fichier inchangés).
    pub invalid_bytes: InvalidBytes,
    /// Les chaînes vides deviennent nulles (ADR 0004 §1).
    pub empty_as_null: bool,
    /// Encodage imposé pour tout le texte (U22 : l'octet 70 n'est pas toujours fiable).
    pub encoding_override: Option<&'static Encoding>,
    /// Lignes visées par lot (un lot s'arrête à la fin de la page qui atteint ce seuil).
    pub batch_rows: usize,
    /// Cas non vérifiés par la spécification (ADR 0006).
    pub strictness: Strictness,
    /// Construction du plan de pages.
    pub plan: PlanMode,
    /// Taille visée (octets) d'un groupe de pages lu en une fois.
    pub group_bytes: u64,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            trailing_blanks: TrailingBlanks::default(),
            missing: MissingMode::default(),
            missing_suffix: "__missing".to_owned(),
            dates: DateMode::default(),
            timestamp_unit: TimestampUnit::default(),
            time_mode: TimeMode::default(),
            out_of_range: OutOfRange::default(),
            invalid_bytes: InvalidBytes::Replace,
            empty_as_null: false,
            encoding_override: None,
            batch_rows: 8192,
            strictness: Strictness::Lenient,
            plan: PlanMode::Fast,
            group_bytes: 8 * 1024 * 1024,
        }
    }
}

impl ReadOptions {
    /// Options d'ouverture cohérentes avec ces options de lecture (encodage imposé, rigueur).
    #[must_use]
    pub fn open_options(&self) -> OpenOptions {
        OpenOptions {
            strictness: self.strictness,
            encoding_override: self.encoding_override,
            ..OpenOptions::default()
        }
    }
}

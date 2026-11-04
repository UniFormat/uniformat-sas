//! Erreurs typées du cœur (ADR 0007 §1). Le `Display` ne cite jamais de données brutes du fichier.

/// Résultat du cœur.
pub type Result<T> = std::result::Result<T, Error>;

/// Erreur d'analyse ou de lecture.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Le contenu n'est pas une table `.sas7bdat` reconnue.
    #[error("pas un fichier .sas7bdat : {reason}")]
    NotSasFile {
        /// Cause (champ ou contrôle en échec).
        reason: &'static str,
    },
    /// Variante du format non prise en charge.
    #[error("format non pris en charge : {what}")]
    UnsupportedFormat {
        /// Variante concernée.
        what: &'static str,
    },
    /// Données insuffisantes.
    #[error("données tronquées : {needed} octets nécessaires, {got} disponibles")]
    Truncated {
        /// Octets nécessaires.
        needed: u64,
        /// Octets disponibles.
        got: u64,
    },
    /// Une taille lue dans le fichier dépasse une borne de `Limits`.
    #[error("limite dépassée : {what} = {value} (limite {limit})")]
    LimitExceeded {
        /// Grandeur concernée.
        what: &'static str,
        /// Valeur lue.
        value: u64,
        /// Borne.
        limit: u64,
    },
    /// Champ d'en-tête invalide.
    #[error("en-tête corrompu : {field} (offset {offset})")]
    CorruptHeader {
        /// Champ.
        field: &'static str,
        /// Offset dans le fichier.
        offset: u64,
    },
    /// Champ de page invalide.
    #[error("page {page} corrompue : {field}")]
    CorruptPage {
        /// Numéro de page (base 0).
        page: u64,
        /// Champ ou structure.
        field: &'static str,
    },
    /// Sous-en-tête invalide.
    #[error("sous-en-tête corrompu en page {page} (signature {signature:#010x})")]
    CorruptSubheader {
        /// Numéro de page (base 0).
        page: u64,
        /// Signature sur 32 bits.
        signature: u32,
    },
    /// Métadonnées incohérentes (références de texte, nombres de colonnes…).
    #[error("métadonnées incohérentes : {what}")]
    CorruptMetadata {
        /// Nature de l'incohérence.
        what: &'static str,
    },
    /// Flot de lignes compressées invalide (RLE ou RDC).
    #[error("flot {algorithm} invalide : {reason}")]
    CorruptCompressed {
        /// `RLE` ou `RDC`.
        algorithm: &'static str,
        /// Nature du défaut.
        reason: &'static str,
    },
    /// Ligne invalide.
    #[error("ligne {row} de la page {page} corrompue")]
    CorruptRow {
        /// Numéro de page (base 0).
        page: u64,
        /// Indice de la ligne dans la page.
        row: u32,
    },
    /// Fonction pas encore prise en charge (jalon ultérieur).
    #[error("non pris en charge : {0}")]
    Unsupported(&'static str),
    /// Code d'encodage inconnu du décodeur.
    #[error("code d'encodage inconnu : {0}")]
    UnknownEncoding(u8),
    /// Octets invalides pour l'encodage de la table (mode `InvalidBytes::Error`).
    #[error("octets invalides pour l'encodage de la colonne {column}, ligne {row}")]
    Encoding {
        /// Rang de la colonne.
        column: usize,
        /// Indice de la ligne physique.
        row: u64,
    },
    /// Mode strict : le cas rencontré n'est pas vérifié par la spécification (ADR 0006).
    #[error("cas non vérifié ({id}) refusé en mode strict")]
    Unverified {
        /// Identifiant de l'inconnue (`U19`…).
        id: &'static str,
    },
}

//! Erreur de la couche d'I/O (ADR 0007 §1) : enveloppe `io::Error` et l'erreur du cœur.

/// Résultat de la couche d'I/O.
pub type Result<T> = std::result::Result<T, IoError>;

/// Erreur de lecture : entrée/sortie ou format.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IoError {
    /// Erreur du système de fichiers ou de la source.
    #[error("erreur d'entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
    /// Erreur d'analyse du format.
    #[error(transparent)]
    Format(#[from] uniformat_sas::Error),
    /// Le plan de pages rapide suppose des pages de données pleines ; une page le contredit (ADR 0002).
    /// Relancer avec le plan complet (`PlanMode::Full`).
    #[error("plan de pages contredit par la page {page} : {expected} lignes attendues, {found} lues (utiliser le plan complet)")]
    PlanMismatch {
        /// Numéro de page (base 0).
        page: u64,
        /// Lignes physiques attendues.
        expected: u64,
        /// Lignes physiques lues.
        found: u64,
    },
}

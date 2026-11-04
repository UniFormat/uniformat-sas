//! Bornes sur toute allocation dérivée d'un champ du fichier (`docs/architecture.md` §6.4).

/// Plafonds appliqués avant toute allocation ou lecture dérivée du fichier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Taille maximale de l'en-tête de fichier.
    pub max_header_length: u32,
    /// Taille maximale d'une page.
    pub max_page_length: u32,
    /// Nombre maximal de colonnes.
    pub max_columns: u32,
    /// Longueur maximale d'une ligne.
    pub max_row_length: u32,
    /// Total maximal des blocs de texte (noms, formats, labels).
    pub max_text_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_header_length: 1 << 20,
            max_page_length: 16 << 20,
            max_columns: 100_000,
            max_row_length: 64 << 20,
            max_text_bytes: 256 << 20,
        }
    }
}

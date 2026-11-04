//! Erreurs de l'adaptateur Arrow (ADR 0007 §1).

/// Résultat de l'adaptateur Arrow.
pub type Result<T> = std::result::Result<T, Error>;

/// Erreur de lecture Arrow.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Erreur d'entrée/sortie ou de format de la couche `uniformat-sas-io`.
    #[error(transparent)]
    Io(#[from] uniformat_sas_io::IoError),
    /// Erreur du cœur.
    #[error(transparent)]
    Format(#[from] uniformat_sas::Error),
    /// Erreur Arrow (construction d'un lot).
    #[error("erreur Arrow : {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    /// Valeur de date, datetime ou heure non représentable (`OutOfRange::Error`).
    #[error("valeur temporelle hors plage : colonne {column}, ligne {row}")]
    OutOfRange {
        /// Rang de la colonne.
        column: usize,
        /// Indice de la ligne physique.
        row: u64,
    },
    /// Erreur d'écriture (schéma, valeur non représentable, entrée/sortie).
    #[error(transparent)]
    Write(#[from] uniformat_sas_io::WriteIoError),
    /// Type Arrow sans équivalent SAS.
    #[error("colonne {column:?} : type Arrow {data_type} non pris en charge à l'écriture")]
    UnsupportedType {
        /// Nom de la colonne.
        column: String,
        /// Type Arrow.
        data_type: String,
    },
    /// Longueur d'une colonne caractère inconnue (ni `sas.length` ni mesure sur les lots).
    #[error(
        "colonne {0:?} : longueur caractère inconnue (métadonnée `sas.length` ou `char_lengths`)"
    )]
    CharLengthUnknown(String),
    /// Indice de projection hors du schéma.
    #[error("projection : colonne {0} hors du schéma")]
    Projection(usize),
    /// Fonction non prise en charge.
    #[error("non pris en charge : {0}")]
    Unsupported(&'static str),
    /// Erreur d'une requête d'objet distant.
    #[cfg(feature = "object-store")]
    #[error("objet distant : {0}")]
    ObjectStore(#[from] object_store::Error),
}

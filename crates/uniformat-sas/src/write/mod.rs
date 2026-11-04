//! Écriture de fichiers `.sas7bdat` (jalon J6, ADR 0005, `20` §4) : tampons d'octets seulement, sans entrée/sortie.
//!
//! [`FileBuilder`] valide le schéma avant le premier octet, encode les lignes une à une et rend les pages au fur et
//! à mesure ; la couche d'E/S (`uniformat-sas-io`) les écrit dans un fichier ou un tampon.

mod builder;
mod meta;
mod types;

pub use builder::{FileBuilder, Finished};
pub use types::{
    CellProblem, CellValue, SchemaError, Unrepresentable, WriteColumn, WriteEncoding, WriteError,
    WriteKind, WriteOptions, WriteReport, WriteSchema,
};

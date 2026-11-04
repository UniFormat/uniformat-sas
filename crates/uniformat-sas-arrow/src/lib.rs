//! Adaptateur Arrow : schéma sans lire les données, lots `RecordBatch`, valeurs manquantes spéciales (ADR 0003).
//!
//! - [`arrow_schema`] : schéma complet depuis les métadonnées seules (en-tête et pages de métadonnées) ;
//! - [`BatchReader`] : lecture synchrone d'une plage de pages, projection et `LIMIT` poussés ;
//! - [`BatchAssembler`] : pages → lots, sans I/O, partagé avec les adaptateurs asynchrones.
#![forbid(unsafe_code)]

mod assembler;
mod batch_budget;
mod error;
mod format;
#[cfg(feature = "object-store")]
mod object;
mod options;
mod reader;
mod schema;
mod writer;

pub use assembler::{BatchAssembler, ReadStats};
pub use error::{Error, Result};
pub use format::{read_options, register, write_options, SasFormat};
#[cfg(feature = "object-store")]
pub use object::ObjectFile;
pub use options::{
    DateMode, MissingMode, OutOfRange, ReadOptions, TimeMode, TimestampUnit, TrailingBlanks,
};
pub use reader::{ArrowBatchReader, BatchReader};
pub use schema::arrow_schema;
pub use writer::{
    infer_char_lengths, write_batches, write_schema_from_arrow, ArrowWriteOptions, ArrowWriter,
};

//! Cœur du driver `sas7bdat` : décrit le format sur des tampons d'octets, sans entrée/sortie.
//!
//! Jalon J1 (lecture non compressée) : [`HeaderParser`] → [`MetaScanner`] → [`PageDecoder`].
//! Dépendances autorisées : liste fermée de l'ADR 0001 (`ci/check_core_deps.sh`).
#![forbid(unsafe_code)]

mod bytes;
pub mod compress;
mod encoding;
mod encoding_table;
mod error;
mod format_table;
mod formats;
mod header;
mod limits;
mod meta;
mod page;
mod report;
pub mod temporal;
pub mod write;

pub use bytes::Endian;
pub use encoding::{encoding_for_code, encoding_info, encodings, EncodingInfo};
pub use encoding_rs::{Encoding, UTF_8};
pub use error::{Error, Result};
pub use formats::{classify_format, is_native_format, LogicalType, StorageKind};
pub use header::{FileHeader, HeaderParser, Platform, MIN_HEADER_BYTES};
pub use limits::Limits;
pub use meta::{Column, ColumnKind, Compression, MetaScanner, SasFormat, ScanProgress, TableMeta};
pub use page::{
    DecoderState, InvalidBytes, MissingKind, NumValue, PageDecoder, PageHeader, PageKind, PageRows,
    RowRef, TextPolicy, TextValue,
};
pub use report::{ReadReport, Strictness, Warning};

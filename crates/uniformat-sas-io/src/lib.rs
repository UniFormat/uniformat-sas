//! Abstraction `ByteSource` (plages d'octets) et lecteur `SasFile` au-dessus du cœur `sas7bdat` (ADR 0002).
//!
//! Jalon J1 : sources mémoire et fichier (lecture positionnée), lecture non compressée. `mmap` et
//! `object_store` arrivent aux jalons suivants.
#![deny(unsafe_code)]

mod error;
mod file;
mod plan;
mod write;

pub use error::{IoError, Result};
pub use file::{OpenOptions, SasFile};
pub use plan::{PageGroup, PageInfo, PagePlan, PlanMode};
// Byte sources and the transactional file now live in the shared core (`uniformat-core`); re-exported here so that
// existing code keeps using `uniformat_sas_io::ByteSource`, `FileSource`…
pub use uniformat_core::{AtomicFile, ByteSource, CountingSource, FileSource, SliceSource};
pub use write::{system_options, SasWriter, WriteIoError};

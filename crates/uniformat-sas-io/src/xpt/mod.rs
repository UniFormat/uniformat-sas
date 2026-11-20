//! Sequential XPORT v5/v8 access, with explicit member indexing and atomic writes.
mod file;
mod metadata;
mod rows;
mod write;
pub use file::{Member, XptFile, XptOpenOptions};
pub use rows::XptRows;
pub use write::XptWriter;

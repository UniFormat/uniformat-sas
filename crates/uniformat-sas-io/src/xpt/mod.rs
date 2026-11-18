//! Sequential XPORT v5 access, with explicit member indexing and atomic writes.
mod file;
mod rows;
mod write;
pub use file::{Member, XptFile, XptOpenOptions};
pub use rows::XptRows;
pub use write::XptWriter;

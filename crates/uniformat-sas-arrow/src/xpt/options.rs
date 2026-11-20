use crate::{OutOfRange, ReadOptions};
use std::collections::HashMap;
use uniformat_sas::{InvalidBytes, Limits};
use uniformat_sas_io::XptOpenOptions;

/// Member choice within a transport library.
#[derive(Debug, Clone, Default)]
pub enum MemberSelection {
    /// First dataset (default).
    #[default]
    First,
    /// Zero-based index.
    Index(usize),
    /// Case-insensitive dataset name; ambiguous names are rejected.
    Name(String),
}
/// Options for bounded XPORT reads.
#[derive(Debug, Clone)]
pub struct XptReadOptions {
    /// Codec limits and explicit character encoding.
    pub open: XptOpenOptions,
    /// Arrow types, missing values, batch size and resource budgets.
    pub read: ReadOptions,
    /// Dataset to select.
    pub member: MemberSelection,
}
impl Default for XptReadOptions {
    fn default() -> Self {
        Self {
            open: XptOpenOptions::default(),
            member: MemberSelection::First,
            read: ReadOptions {
                invalid_bytes: InvalidBytes::Error,
                out_of_range: OutOfRange::Error,
                ..Default::default()
            },
        }
    }
}
/// Options for strict, deterministic XPORT writes.
#[derive(Debug, Clone)]
pub struct XptWriteOptions {
    /// Transport header and encoding options.
    pub write: uniformat_sas::xpt::WriteOptions,
    /// Codec resource limits.
    pub limits: Limits,
    /// Width overrides for character columns, in encoded bytes.
    pub char_lengths: HashMap<String, u32>,
    /// Suffix for numeric missing-value sidecars.
    pub missing_suffix: String,
    /// Maximum retained input buffers when inferring character widths.
    pub max_buffer_bytes: usize,
}
impl Default for XptWriteOptions {
    fn default() -> Self {
        Self {
            write: Default::default(),
            limits: Limits::default(),
            char_lengths: HashMap::new(),
            missing_suffix: "__missing".into(),
            max_buffer_bytes: 256 << 20,
        }
    }
}

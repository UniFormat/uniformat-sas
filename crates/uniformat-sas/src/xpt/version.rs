/// Transport layout, detected from the library marker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Version {
    /// XPORT version 5/6 layout (default for writing).
    #[default]
    V5,
    /// Extended version 8/9 transport layout.
    V8,
}
impl Version {
    pub(super) fn marker(self, v5: &'static str, v8: &'static str) -> &'static str {
        match self {
            Self::V5 => v5,
            Self::V8 => v8,
        }
    }
    pub(super) fn name_len(self) -> usize {
        match self {
            Self::V5 => 8,
            Self::V8 => 32,
        }
    }
    pub(super) fn columns(self) -> usize {
        match self {
            Self::V5 => 9999,
            Self::V8 => 32767,
        }
    }
    pub(super) fn char_len(self) -> u32 {
        match self {
            Self::V5 => 200,
            Self::V8 => 32767,
        }
    }
    /// Numeric transport version, suitable for metadata.
    pub fn number(self) -> u8 {
        match self {
            Self::V5 => 5,
            Self::V8 => 8,
        }
    }
}

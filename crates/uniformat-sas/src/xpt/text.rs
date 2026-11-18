use super::invalid;
use crate::Result;
use std::borrow::Cow;

/// Explicit encoding: XPORT does not record a character encoding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextEncoding {
    /// Strict UTF-8 (default).
    #[default]
    Utf8,
    /// ISO-8859-1, including control characters at 0x80–0x9f.
    Latin1,
    /// Windows-1252 (different from ISO-8859-1).
    Windows1252,
}

impl TextEncoding {
    /// Decode without replacing invalid input.
    pub fn decode(self, data: &[u8]) -> Result<Cow<'_, str>> {
        match self {
            Self::Utf8 => std::str::from_utf8(data)
                .map(Cow::Borrowed)
                .map_err(|_| invalid("XPT invalid UTF-8")),
            Self::Latin1 => Ok(Cow::Owned(data.iter().map(|&b| char::from(b)).collect())),
            Self::Windows1252 => encoding_rs::WINDOWS_1252
                .decode_without_bom_handling_and_without_replacement(data)
                .ok_or_else(|| invalid("XPT invalid Windows-1252")),
        }
    }

    pub(super) fn encode(self, text: &str) -> Result<Cow<'_, [u8]>> {
        let enc = match self {
            Self::Utf8 => crate::write::WriteEncoding::Utf8,
            Self::Latin1 => crate::write::WriteEncoding::Latin1,
            Self::Windows1252 => crate::write::WriteEncoding::Wlatin1,
        };
        enc.encode(text)
            .ok_or_else(|| invalid("XPT unencodable text"))
    }

    pub(super) fn field(self, data: &[u8]) -> Result<String> {
        let end = data
            .iter()
            .rposition(|&b| b != b' ')
            .map_or(0, |n| n.saturating_add(1));
        Ok(self.decode(super::bytes(data, 0, end)?)?.into_owned())
    }
}

//! Encodages (`30` §6) : table native indexée par le code de l'octet 70 et décodage du texte.
//!
//! La table (`encoding_table.rs`, générée) reprend les 225 codes nommés du module `tknlsenc` et, pour les 127 codes
//! de `30` §6.1, l'encodage standard (WHATWG / `encoding_rs`) quand il existe. Les pages de code IBM, EBCDIC, EUC-TW,
//! ISO-2022 et quelques Mac n'ont pas d'équivalent : leur texte se lit en octets bruts (`InvalidBytes::Binary`).

use encoding_rs::Encoding;

use crate::encoding_table::ENCODING_TABLE;

/// Un code d'encodage de la table native.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodingInfo {
    /// Code de l'octet 70 de l'en-tête.
    pub code: u8,
    /// Nom SAS (vide pour le code 0 : encodage non déclaré).
    pub sas_name: &'static str,
    /// Nom de type iconv de la table native.
    pub iconv_name: &'static str,
    /// Label `encoding_rs` de l'encodage équivalent, s'il existe.
    pub whatwg: Option<&'static str>,
    /// Octet de remplissage de la table native (`0x20`, ou `0x40` pour EBCDIC).
    pub blank: u8,
}

/// Les codes nommés de la table native, triés par code.
pub fn encodings() -> &'static [EncodingInfo] {
    ENCODING_TABLE
}

/// Fiche d'un code d'encodage.
pub fn encoding_info(code: u8) -> Option<&'static EncodingInfo> {
    ENCODING_TABLE
        .binary_search_by_key(&code, |e| e.code)
        .ok()
        .and_then(|i| ENCODING_TABLE.get(i))
}

/// Encodage de décodage pour un code d'en-tête (octet 70), s'il existe un équivalent standard.
///
/// Réserves (ADR 0001) : `encoding_rs` n'a ni US-ASCII ni ISO-8859-1 stricts ; 28 et 29 sont lus comme
/// `windows-1252` (les octets 0x80 à 0x9F diffèrent d'ISO-8859-1), 37 comme `windows-1254`. Le code 0 (non
/// déclaré) et 204 (`any`) se lisent comme `windows-1252`.
pub fn encoding_for_code(code: u8) -> Option<&'static Encoding> {
    encoding_info(code)
        .and_then(|e| e.whatwg)
        .and_then(|label| Encoding::for_label(label.as_bytes()))
}

/// Octet de blanc de remplissage des champs caractère : `0x40` pour les encodages EBCDIC, `0x20` sinon.
pub(crate) fn blank_byte(code: u8) -> u8 {
    match encoding_info(code) {
        Some(e) if e.blank == 0x40 => 0x40,
        _ => 0x20,
    }
}

/// Retire les blancs et NUL de fin d'un texte déjà décodé.
pub(crate) fn trim_text(s: &str) -> &str {
    s.trim_end_matches([' ', '\0'])
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_by_code() {
        assert!(ENCODING_TABLE.windows(2).all(|w| w[0].code < w[1].code));
        assert_eq!(ENCODING_TABLE.len(), 226);
    }

    #[test]
    fn observed_codes_map_to_expected_encodings() {
        assert_eq!(encoding_for_code(20), Some(encoding_rs::UTF_8));
        assert_eq!(encoding_for_code(29), Some(encoding_rs::WINDOWS_1252));
        assert_eq!(encoding_for_code(62), Some(encoding_rs::WINDOWS_1252));
        assert_eq!(encoding_for_code(138), Some(encoding_rs::SHIFT_JIS));
        assert_eq!(encoding_for_code(0), Some(encoding_rs::WINDOWS_1252));
        assert_eq!(encoding_for_code(125).map(Encoding::name), Some("GBK"));
        assert_eq!(encoding_for_code(205).map(Encoding::name), Some("gb18030"));
    }

    #[test]
    fn codes_without_standard_equivalent_have_none() {
        assert!(encoding_info(247).is_some());
        assert_eq!(encoding_for_code(247), None);
        assert_eq!(encoding_for_code(1), None);
        assert_eq!(encoding_info(255), None);
    }

    #[test]
    fn every_whatwg_label_resolves() {
        for e in encodings() {
            if let Some(label) = e.whatwg {
                assert!(
                    Encoding::for_label(label.as_bytes()).is_some(),
                    "code {} label {label}",
                    e.code
                );
            }
        }
    }
}

//! En-tête de fichier (`10` §2).

use crate::bytes::{slice, Endian, Layout};
use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::report::ReadReport;

/// Nombre minimal d'octets à fournir à [`HeaderParser::parse`] (les champs décodés tiennent dans les 340 premiers).
pub const MIN_HEADER_BYTES: usize = 512;

const MAGIC: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xC2, 0xEA, 0x81, 0x60, 0xB3, 0x14, 0x11, 0xCF, 0xBD, 0x92,
    0x08, 0x00, 0x09, 0xC7, 0x31, 0x8C, 0x18, 0x1F, 0x10, 0x11,
];

/// Famille de système de l'écrivain (octets 39 et 40).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// `'1'`, code 1.
    Unix,
    /// `'2'`, code 4.
    Windows,
    /// Autre valeur de l'octet 39.
    Other(u8),
}

/// En-tête de fichier décodé.
#[derive(Debug, Clone, PartialEq)]
pub struct FileHeader {
    /// Format 64 bits (`true`) ou 32 bits.
    pub is_64bit: bool,
    /// Ordre des octets.
    pub endian: Endian,
    /// Famille de système de l'écrivain.
    pub platform: Platform,
    /// Décalage `a1` (0 ou 4) des champs après l'offset 164.
    pub a1: u8,
    /// Code d'encodage (octet 70).
    pub encoding_code: u8,
    /// Taille de l'en-tête (début de la page 0).
    pub header_length: u32,
    /// Taille d'une page.
    pub page_length: u32,
    /// Nombre de pages après l'en-tête.
    pub page_count: u64,
    /// Création, en secondes depuis le 1er janvier 1960 (heure locale de l'écrivain).
    pub created: f64,
    /// Dernière modification, mêmes unités.
    pub modified: f64,
    /// Décalage UTC de l'écrivain à la création, en secondes (`+7200` = UTC+2).
    pub created_tz_offset: f64,
    /// Décalage UTC de l'écrivain à la modification, en secondes.
    pub modified_tz_offset: f64,
    /// Version de SAS (`9.0401M7`).
    pub sas_release: String,
    /// Famille de système (`Linux`, `WIN`…).
    pub host: String,
    /// Version du système de l'écrivain (tronquée à 16 octets).
    pub os_version: String,
    /// Nom du système de l'écrivain.
    pub os_name: String,
    /// Architecture de l'écrivain (`x86_64`).
    pub os_machine: String,
    /// Nom du membre (vide dans les tables écrites par une étape DATA).
    pub table_name: String,
    /// Type de membre (`DATA`, `INDEX`).
    pub file_type: String,
    /// Valeur `file_code` (0 si absente) : les signatures de page valent `file_code ^ (k + 1)`.
    pub file_code: u32,
}

impl FileHeader {
    /// Création en microsecondes depuis 1970-01-01T00:00:00, **heure locale de l'écrivain** (non convertie).
    pub fn created_local_unix_micros(
        &self,
    ) -> std::result::Result<i64, crate::temporal::ConvertError> {
        crate::temporal::sas_datetime_to_unix_micros(self.created)
    }

    /// Dernière modification, mêmes unités.
    pub fn modified_local_unix_micros(
        &self,
    ) -> std::result::Result<i64, crate::temporal::ConvertError> {
        crate::temporal::sas_datetime_to_unix_micros(self.modified)
    }

    pub(crate) fn layout(&self) -> Layout {
        Layout {
            endian: self.endian,
            w64: self.is_64bit,
        }
    }
}

/// Analyseur d'en-tête.
#[derive(Debug, Clone, Copy)]
pub struct HeaderParser;

fn at(base: usize, delta: usize) -> usize {
    base.saturating_add(delta)
}

fn text(bytes: &[u8], off: usize, len: usize) -> String {
    let raw = slice(bytes, off, len).unwrap_or_default();
    String::from_utf8_lossy(raw)
        .trim_end_matches([' ', '\0'])
        .to_owned()
}

impl HeaderParser {
    /// Analyse les premiers octets du fichier (au moins [`MIN_HEADER_BYTES`], en pratique `header_length`).
    ///
    /// # Errors
    /// `NotSasFile` si le nombre magique ou un descripteur est invalide, `Truncated` si moins de
    /// [`MIN_HEADER_BYTES`] octets sont fournis, `LimitExceeded`/`CorruptHeader` pour les tailles hors bornes.
    pub fn parse(bytes: &[u8], limits: &Limits, report: &mut ReadReport) -> Result<FileHeader> {
        if bytes.len() < MIN_HEADER_BYTES {
            return Err(Error::Truncated {
                needed: MIN_HEADER_BYTES as u64,
                got: bytes.len() as u64,
            });
        }
        if bytes.get(..32) != Some(&MAGIC[..]) {
            return Err(Error::NotSasFile {
                reason: "nombre magique",
            });
        }
        let byte = |i: usize| bytes.get(i).copied().unwrap_or(0);
        let is_64bit = match byte(32) {
            0x33 => true,
            0x22 => false,
            _ => {
                return Err(Error::NotSasFile {
                    reason: "octet 32 (largeur de mot)",
                })
            }
        };
        let a1: usize = if byte(35) == 0x33 { 4 } else { 0 };
        let endian = match byte(37) {
            1 => Endian::Little,
            0 => Endian::Big,
            _ => {
                return Err(Error::NotSasFile {
                    reason: "octet 37 (ordre des octets)",
                })
            }
        };
        let platform = match byte(39) {
            b'1' => Platform::Unix,
            b'2' => Platform::Windows,
            other => Platform::Other(other),
        };
        let layout = Layout {
            endian,
            w64: is_64bit,
        };
        let t = if is_64bit { at(a1, 4) } else { a1 };

        // U01 : le descripteur 32-40 est recopié en 56-64.
        if slice(bytes, 32, 9) != slice(bytes, 56, 9) {
            report.warn(
                "U01",
                None,
                "copie du descripteur (octets 56-64) différente des octets 32-40",
            )?;
        }

        let hl_off = at(196, a1);
        let header_length = endian.u32(bytes, hl_off).ok_or(Error::CorruptHeader {
            field: "header_length",
            offset: hl_off as u64,
        })?;
        let pl_off = at(200, a1);
        let page_length = endian.u32(bytes, pl_off).ok_or(Error::CorruptHeader {
            field: "page_length",
            offset: pl_off as u64,
        })?;
        let pc_off = at(204, a1);
        let page_count = layout.word(bytes, pc_off).ok_or(Error::CorruptHeader {
            field: "page_count",
            offset: pc_off as u64,
        })?;

        let min_header = MIN_HEADER_BYTES as u32;
        if header_length < min_header {
            return Err(Error::CorruptHeader {
                field: "header_length",
                offset: hl_off as u64,
            });
        }
        if header_length > limits.max_header_length {
            return Err(Error::LimitExceeded {
                what: "header_length",
                value: u64::from(header_length),
                limit: u64::from(limits.max_header_length),
            });
        }
        // U23 : page quelconque entre 256 et la borne, multiple de 8.
        if page_length < 256 || page_length % 8 != 0 {
            return Err(Error::CorruptHeader {
                field: "page_length",
                offset: pl_off as u64,
            });
        }
        if page_length > limits.max_page_length {
            return Err(Error::LimitExceeded {
                what: "page_length",
                value: u64::from(page_length),
                limit: u64::from(limits.max_page_length),
            });
        }

        let created = endian.f64(bytes, at(164, a1)).unwrap_or(0.0);
        let modified = endian.f64(bytes, at(172, a1)).unwrap_or(0.0);
        let created_tz_offset = endian.f64(bytes, at(180, a1)).unwrap_or(0.0);
        let modified_tz_offset = endian.f64(bytes, at(188, a1)).unwrap_or(0.0);
        let file_code = endian.u32(bytes, at(320, t)).unwrap_or(0);

        Ok(FileHeader {
            is_64bit,
            endian,
            platform,
            a1: u8::try_from(a1).unwrap_or(0),
            encoding_code: byte(70),
            header_length,
            page_length,
            page_count,
            created,
            modified,
            created_tz_offset,
            modified_tz_offset,
            sas_release: text(bytes, at(216, t), 8),
            host: text(bytes, at(224, t), 16),
            os_version: text(bytes, at(240, t), 16),
            os_name: text(bytes, at(256, t), 16),
            os_machine: text(bytes, at(272, t), 16),
            table_name: text(bytes, 92, 64),
            file_type: text(bytes, 156, 8),
            file_code,
        })
    }
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
    use crate::report::Strictness;

    fn parse(bytes: &[u8]) -> Result<FileHeader> {
        HeaderParser::parse(
            bytes,
            &Limits::default(),
            &mut ReadReport::new(Strictness::Lenient),
        )
    }

    fn minimal() -> Vec<u8> {
        let mut b = vec![0u8; 1024];
        b[..32].copy_from_slice(&MAGIC);
        b[32] = 0x33;
        b[35] = 0x33;
        b[37] = 1;
        b[39] = b'1';
        b[70] = 20;
        b[200..204].copy_from_slice(&1024u32.to_le_bytes()); // header_length à 196 + a1
        b[204..208].copy_from_slice(&65536u32.to_le_bytes()); // page_length à 200 + a1
        b[208..216].copy_from_slice(&3u64.to_le_bytes()); // page_count à 204 + a1
        b
    }

    #[test]
    fn minimal_header_is_decoded() {
        let h = parse(&minimal()).unwrap();
        assert!(h.is_64bit);
        assert_eq!(h.endian, Endian::Little);
        assert_eq!(
            (h.header_length, h.page_length, h.page_count),
            (1024, 65536, 3)
        );
        assert_eq!(h.platform, Platform::Unix);
        assert_eq!(h.encoding_code, 20);
    }

    #[test]
    fn rejects_short_input_bad_magic_and_bad_flags() {
        assert!(matches!(parse(&[0; 100]), Err(Error::Truncated { .. })));
        let mut b = minimal();
        b[15] = 0x65; // fichier d'index
        assert!(matches!(parse(&b), Err(Error::NotSasFile { .. })));
        let mut b = minimal();
        b[32] = 0x44;
        assert!(matches!(parse(&b), Err(Error::NotSasFile { .. })));
        let mut b = minimal();
        b[37] = 7;
        assert!(matches!(parse(&b), Err(Error::NotSasFile { .. })));
    }

    #[test]
    fn rejects_out_of_range_sizes() {
        let mut b = minimal();
        b[204..208].copy_from_slice(&100u32.to_le_bytes());
        assert!(matches!(
            parse(&b),
            Err(Error::CorruptHeader {
                field: "page_length",
                ..
            })
        ));
        let mut b = minimal();
        b[204..208].copy_from_slice(&(64u32 << 20).to_le_bytes());
        assert!(matches!(
            parse(&b),
            Err(Error::LimitExceeded {
                what: "page_length",
                ..
            })
        ));
        let mut b = minimal();
        b[200..204].copy_from_slice(&16u32.to_le_bytes());
        assert!(matches!(
            parse(&b),
            Err(Error::CorruptHeader {
                field: "header_length",
                ..
            })
        ));
    }
}

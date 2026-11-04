//! Types publics de l'écriture : schéma, options, valeurs de cellule, erreurs, rapport (ADR 0005).

use std::borrow::Cow;

use crate::{Compression, MissingKind, SasFormat};

/// Encodages écrits (`20` §3) : UTF-8 (code 20), latin1 (29), wlatin1 (62).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteEncoding {
    /// UTF-8 (code 20).
    #[default]
    Utf8,
    /// ISO-8859-1 (code 29).
    Latin1,
    /// Windows-1252 (code 62).
    Wlatin1,
}

impl WriteEncoding {
    /// Code d'encodage de l'octet 70 de l'en-tête.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Utf8 => 20,
            Self::Latin1 => 29,
            Self::Wlatin1 => 62,
        }
    }

    /// Encodage correspondant à un code d'en-tête, s'il est écrit.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            20 => Some(Self::Utf8),
            29 => Some(Self::Latin1),
            62 => Some(Self::Wlatin1),
            _ => None,
        }
    }

    /// Octets de `s`, ou `None` si un caractère n'a pas d'équivalent dans l'encodage.
    #[must_use]
    pub fn encode(self, s: &str) -> Option<Cow<'_, [u8]>> {
        match self {
            Self::Utf8 => Some(Cow::Borrowed(s.as_bytes())),
            Self::Latin1 => {
                let mut out = Vec::with_capacity(s.len());
                for c in s.chars() {
                    out.push(u8::try_from(u32::from(c)).ok()?);
                }
                Some(Cow::Owned(out))
            }
            Self::Wlatin1 => {
                let (bytes, _, had_errors) = encoding_rs::WINDOWS_1252.encode(s);
                (!had_errors).then(|| Cow::Owned(bytes.into_owned()))
            }
        }
    }

    /// Octets de `s` ; un caractère non encodable devient `?`. Rend aussi le nombre de caractères remplacés.
    #[must_use]
    pub fn encode_lossy(self, s: &str) -> (Cow<'_, [u8]>, u64) {
        if let Some(b) = self.encode(s) {
            return (b, 0);
        }
        let mut out = Vec::with_capacity(s.len());
        let mut replaced = 0u64;
        let mut buf = [0u8; 4];
        for c in s.chars() {
            match self.encode(c.encode_utf8(&mut buf)) {
                Some(b) => out.extend_from_slice(&b),
                None => {
                    out.push(b'?');
                    replaced = replaced.saturating_add(1);
                }
            }
        }
        (Cow::Owned(out), replaced)
    }
}

/// Type et longueur de stockage d'une colonne écrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteKind {
    /// Numérique de 3 à 8 octets.
    Numeric {
        /// Longueur de stockage.
        length: u8,
    },
    /// Caractère de 1 à 32 767 octets.
    Char {
        /// Longueur de stockage en octets (dans l'encodage cible).
        length: u32,
    },
}

/// Une colonne à écrire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteColumn {
    /// Nom (32 octets au plus dans l'encodage cible).
    pub name: String,
    /// Type et longueur.
    pub kind: WriteKind,
    /// Label de variable (256 octets au plus), vide si absent.
    pub label: String,
    /// Format de sortie.
    pub format: Option<SasFormat>,
    /// Informat.
    pub informat: Option<SasFormat>,
}

impl WriteColumn {
    /// Colonne numérique de `length` octets (8 pour un `double` entier).
    #[must_use]
    pub fn numeric(name: impl Into<String>, length: u8) -> Self {
        Self {
            name: name.into(),
            kind: WriteKind::Numeric { length },
            label: String::new(),
            format: None,
            informat: None,
        }
    }

    /// Colonne caractère de `length` octets.
    #[must_use]
    pub fn character(name: impl Into<String>, length: u32) -> Self {
        Self {
            name: name.into(),
            kind: WriteKind::Char { length },
            label: String::new(),
            format: None,
            informat: None,
        }
    }

    /// Ajoute un label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Ajoute un format de sortie (`DATE`, 9, 0).
    #[must_use]
    pub fn with_format(mut self, name: impl Into<String>, width: u16, decimals: u16) -> Self {
        self.format = Some(SasFormat {
            name: name.into(),
            width,
            decimals,
        });
        self
    }

    /// Ajoute un informat.
    #[must_use]
    pub fn with_informat(mut self, name: impl Into<String>, width: u16, decimals: u16) -> Self {
        self.informat = Some(SasFormat {
            name: name.into(),
            width,
            decimals,
        });
        self
    }

    /// Longueur de stockage en octets.
    #[must_use]
    pub fn length(&self) -> usize {
        match self.kind {
            WriteKind::Numeric { length } => usize::from(length),
            WriteKind::Char { length } => usize::try_from(length).unwrap_or(usize::MAX),
        }
    }
}

/// Schéma d'une table à écrire.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteSchema {
    /// Colonnes, dans l'ordre de création.
    pub columns: Vec<WriteColumn>,
}

/// Que faire d'une valeur non représentable (ADR 0005 §4) ?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unrepresentable {
    /// Abandonner l'écriture avec la position (colonne, ligne) (défaut).
    #[default]
    Error,
    /// Remplacer : `?` pour un caractère non encodable, `.` pour une valeur infinie, arrondi pour un entier trop grand,
    /// troncature de poids faible pour une mantisse trop longue.
    Replace,
    /// Tronquer un texte trop long à la limite d'un caractère.
    Truncate,
}

/// Options d'écriture.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Encodage du fichier.
    pub encoding: WriteEncoding,
    /// Taille de page demandée (4 096 à 1 048 576 relus par SAS).
    pub page_length: u32,
    /// Agrandir la page si les métadonnées ou une ligne n'y tiennent pas (jusqu'à 1 Mio) ; sinon erreur typée.
    pub grow_page: bool,
    /// Compression des lignes.
    pub compression: Compression,
    /// Date de création, en secondes depuis 1960-01-01 (heure locale de l'écrivain, décalage nul).
    pub created: f64,
    /// Code de fichier : la page `k` est signée `file_code XOR (k + 1)`.
    pub file_code: u32,
    /// Release écrite (`Release Created`).
    pub release: String,
    /// Hôte écrit (`Host Created`).
    pub host: String,
    /// Version du système (16 octets au plus).
    pub os_version: String,
    /// Machine (6 octets au plus).
    pub os_machine: String,
    /// Nom de la table (32 octets au plus ; SAS affiche le nom du fichier).
    pub table_name: String,
    /// Traitement des valeurs non représentables.
    pub on_unrepresentable: Unrepresentable,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            encoding: WriteEncoding::Utf8,
            page_length: 65_536,
            grow_page: true,
            compression: Compression::None,
            created: 0.0,
            file_code: 0,
            release: "9.0401M7".to_owned(),
            host: "Linux".to_owned(),
            os_version: "6.18.48-107.148.".to_owned(),
            os_machine: "x86_64".to_owned(),
            table_name: String::new(),
            on_unrepresentable: Unrepresentable::Error,
        }
    }
}

/// Valeur d'une cellule à écrire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellValue<'a> {
    /// Nombre (`NaN` s'écrit `.`).
    Num(f64),
    /// Manquant numérique spécial.
    Missing(MissingKind),
    /// Texte (encodé dans l'encodage cible).
    Text(&'a str),
    /// Octets bruts d'une colonne caractère.
    Bytes(&'a [u8]),
    /// Absence de valeur : `.` pour un numérique, blancs pour un caractère.
    Null,
}

/// Limite de schéma dépassée ou schéma invalide : détectée avant le premier octet écrit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaError {
    /// Aucune colonne.
    #[error("la table n'a aucune colonne")]
    NoColumns,
    /// Nom vide.
    #[error("colonne {index} : nom vide")]
    EmptyName {
        /// Rang de la colonne.
        index: usize,
    },
    /// Nom de plus de 32 octets dans l'encodage cible.
    #[error("colonne {name:?} : nom de {bytes} octets (maximum {max})")]
    NameTooLong {
        /// Nom.
        name: String,
        /// Longueur en octets.
        bytes: usize,
        /// Limite.
        max: usize,
    },
    /// Deux colonnes portent le même nom.
    #[error("nom de colonne en double : {name:?}")]
    DuplicateName {
        /// Nom.
        name: String,
    },
    /// Label de plus de 256 octets.
    #[error("colonne {name:?} : label de {bytes} octets (maximum {max})")]
    LabelTooLong {
        /// Nom de la colonne.
        name: String,
        /// Longueur en octets.
        bytes: usize,
        /// Limite.
        max: usize,
    },
    /// Texte (nom, label, nom de format) impossible à coder dans l'encodage cible.
    #[error("colonne {name:?} : texte non encodable dans l'encodage cible")]
    UnencodableText {
        /// Nom de la colonne.
        name: String,
    },
    /// Colonne caractère hors de 1 à 32 767 octets.
    #[error("colonne {name:?} : longueur caractère {length} hors de 1 à {max}")]
    CharLength {
        /// Nom de la colonne.
        name: String,
        /// Longueur demandée.
        length: u32,
        /// Limite.
        max: u32,
    },
    /// Colonne numérique hors de 3 à 8 octets.
    #[error("colonne {name:?} : longueur numérique {length} hors de 3 à 8")]
    NumericLength {
        /// Nom de la colonne.
        name: String,
        /// Longueur demandée.
        length: u8,
    },
    /// Les métadonnées ne tiennent pas sur la page 0 (plusieurs pages de métadonnées non écrites, W04).
    #[error("{columns} colonnes : les métadonnées ne tiennent pas sur la page 0 de {page_length} octets (maximum {max_for_page} colonnes)")]
    TooManyColumns {
        /// Nombre de colonnes.
        columns: usize,
        /// Taille de page.
        page_length: u32,
        /// Nombre maximal de colonnes pour cette page.
        max_for_page: usize,
    },
    /// Une ligne ne tient pas dans une page.
    #[error("ligne de {row_length} octets : ne tient pas dans une page de {page_length} octets")]
    RowTooLong {
        /// Longueur de ligne.
        row_length: usize,
        /// Taille de page.
        page_length: u32,
    },
    /// Taille de page hors de 1 024 à 1 048 576 octets.
    #[error("taille de page {page_length} hors de 1024 à 1048576")]
    PageLength {
        /// Taille demandée.
        page_length: u32,
    },
}

/// Nature du défaut d'une cellule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CellProblem {
    /// Texte plus long que la colonne.
    TextTooLong {
        /// Longueur du texte encodé.
        bytes: usize,
        /// Longueur de la colonne.
        max: usize,
    },
    /// Caractère sans équivalent dans l'encodage cible.
    Unencodable,
    /// Entier de plus de 53 bits, non représentable exactement par un `double`.
    IntegerTooLarge,
    /// Valeur infinie.
    Infinite,
    /// Date ou date-heure hors de la plage de SAS.
    OutOfRange,
    /// Mantisse tronquée par une longueur numérique de moins de 8 octets.
    Narrowed,
    /// Type de valeur incompatible avec la colonne.
    WrongType,
}

impl std::fmt::Display for CellProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TextTooLong { bytes, max } => {
                write!(f, "texte de {bytes} octets pour une colonne de {max}")
            }
            Self::Unencodable => f.write_str("caractère non encodable dans l'encodage cible"),
            Self::IntegerTooLarge => f.write_str("entier au-delà de 2^53"),
            Self::Infinite => f.write_str("valeur infinie"),
            Self::OutOfRange => f.write_str("date ou date-heure hors plage"),
            Self::Narrowed => f.write_str("mantisse tronquée par la longueur de la colonne"),
            Self::WrongType => f.write_str("type de valeur incompatible avec la colonne"),
        }
    }
}

/// Erreur d'écriture.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// Schéma invalide ou limite dépassée.
    #[error(transparent)]
    Schema(#[from] SchemaError),
    /// Valeur non représentable (politique `Unrepresentable::Error`).
    #[error("colonne {column}, ligne {row} : {problem}")]
    Cell {
        /// Rang de la colonne.
        column: usize,
        /// Rang de la ligne (base 0).
        row: u64,
        /// Défaut constaté.
        problem: CellProblem,
    },
    /// Nombre de cellules différent du nombre de colonnes.
    #[error("ligne {row} : {got} cellules pour {expected} colonnes")]
    RowShape {
        /// Rang de la ligne.
        row: u64,
        /// Colonnes du schéma.
        expected: usize,
        /// Cellules reçues.
        got: usize,
    },
    /// Une ligne compressée ne tient pas dans une page.
    #[error("ligne {row} : flot compressé de {bytes} octets, trop long pour une page de {page_length} octets")]
    CompressedRowTooLong {
        /// Rang de la ligne.
        row: u64,
        /// Longueur du flot.
        bytes: usize,
        /// Taille de page.
        page_length: u32,
    },
    /// Fonction non prise en charge (ADR 0005 §5, `20` §7).
    #[error("non pris en charge : {0}")]
    Unsupported(&'static str),
}

/// Écarts comptés pendant l'écriture (jamais silencieux, ADR 0005).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteReport {
    /// Lignes écrites.
    pub rows: u64,
    /// Textes tronqués à la limite d'un caractère.
    pub truncated_values: u64,
    /// Caractères remplacés par `?`.
    pub replaced_chars: u64,
    /// Valeurs infinies remplacées par `.`.
    pub replaced_values: u64,
    /// Entiers arrondis au `double` le plus proche (appelé par la couche Arrow).
    pub inexact_integers: u64,
    /// Mantisses tronquées par une longueur numérique inférieure à 8.
    pub narrowed_values: u64,
    /// Avertissements (`compression_disabled_small_rows`, `page_length_grown`, `name_case_collision`…).
    pub warnings: Vec<String>,
}

impl WriteReport {
    /// Vrai si une valeur a été modifiée (tronquée, remplacée, arrondie).
    #[must_use]
    pub fn altered_values(&self) -> bool {
        self.truncated_values
            .saturating_add(self.replaced_chars)
            .saturating_add(self.replaced_values)
            .saturating_add(self.inexact_integers)
            .saturating_add(self.narrowed_values)
            > 0
    }
}

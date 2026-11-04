//! Lectures d'entiers bornées : aucune indexation directe, aucune arithmétique non vérifiée (ADR 0007 §1).

/// Ordre des octets du fichier (octet 37 de l'en-tête).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Octet de poids faible en premier.
    Little,
    /// Octet de poids fort en premier.
    Big,
}

/// Tranche `[off, off + len)` si elle est entièrement dans `buf`.
pub(crate) fn slice(buf: &[u8], off: usize, len: usize) -> Option<&[u8]> {
    let end = off.checked_add(len)?;
    buf.get(off..end)
}

pub(crate) fn arr<const N: usize>(buf: &[u8], off: usize) -> Option<[u8; N]> {
    slice(buf, off, N)?.try_into().ok()
}

pub(crate) fn to_usize(v: u64) -> Option<usize> {
    usize::try_from(v).ok()
}

impl Endian {
    pub(crate) fn u16(self, buf: &[u8], off: usize) -> Option<u16> {
        let a = arr::<2>(buf, off)?;
        Some(match self {
            Self::Little => u16::from_le_bytes(a),
            Self::Big => u16::from_be_bytes(a),
        })
    }

    pub(crate) fn u32(self, buf: &[u8], off: usize) -> Option<u32> {
        let a = arr::<4>(buf, off)?;
        Some(match self {
            Self::Little => u32::from_le_bytes(a),
            Self::Big => u32::from_be_bytes(a),
        })
    }

    pub(crate) fn u64(self, buf: &[u8], off: usize) -> Option<u64> {
        let a = arr::<8>(buf, off)?;
        Some(match self {
            Self::Little => u64::from_le_bytes(a),
            Self::Big => u64::from_be_bytes(a),
        })
    }

    pub(crate) fn f64(self, buf: &[u8], off: usize) -> Option<f64> {
        self.u64(buf, off).map(f64::from_bits)
    }
}

/// Largeur de mot (`w`) et ordre des octets : tout ce qu'il faut pour lire les structures du fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) endian: Endian,
    pub(crate) w64: bool,
}

impl Layout {
    /// Taille d'un mot (`w`) : 8 ou 4 octets.
    pub(crate) fn w(self) -> usize {
        if self.w64 {
            8
        } else {
            4
        }
    }

    /// Taille d'un pointeur de sous-en-tête : 24 ou 12 octets.
    pub(crate) fn ps(self) -> usize {
        if self.w64 {
            24
        } else {
            12
        }
    }

    /// Position du type de page (`po`) : 32 ou 16.
    pub(crate) fn po(self) -> usize {
        if self.w64 {
            32
        } else {
            16
        }
    }

    /// Mot de `w` octets à `off`.
    pub(crate) fn word(self, buf: &[u8], off: usize) -> Option<u64> {
        if self.w64 {
            self.endian.u64(buf, off)
        } else {
            self.endian.u32(buf, off).map(u64::from)
        }
    }
}

//! Liaison R (extendr) : lecture d'un `.sas7bdat` en `data.frame` aux attributs `haven` (ADR 0003, ADR 0007).
//!
//! - [`convert`] : conversion en colonnes R, du Rust pur testé sans R ;
//! - `glue` (feature `r`) : fonctions exportées vers R par extendr, seule partie qui contient du code généré `unsafe`.
#![deny(unsafe_code)]

pub mod convert;

#[cfg(feature = "r")]
#[allow(unsafe_code)]
// frontière FFI générée par extendr (ADR 0007 §2) ; aucun bloc `unsafe` écrit à la main
mod glue;

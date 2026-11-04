//! Avertissements non fatals de la lecture (ADR 0006) : un identifiant d'inconnue de `60_inconnues.md`.

use crate::error::{Error, Result};

/// Nombre maximal d'avertissements conservés ; les suivants sont seulement comptés.
const MAX_WARNINGS: usize = 256;

/// Mode de gestion des inconnues (ADR 0006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strictness {
    /// Tolérance et avertissement (défaut).
    #[default]
    Lenient,
    /// Les cas non vérifiés (`U19`, `U20`, `U21`, `U24`, `C09`) sont des erreurs.
    Strict,
}

/// Un avertissement typé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Identifiant de l'inconnue (`U06`, `U19`…).
    pub id: &'static str,
    /// Page concernée, si l'avertissement s'y rattache.
    pub page: Option<u64>,
    /// Description fixe (jamais de donnée brute du fichier).
    pub detail: &'static str,
}

/// Avertissements collectés pendant la lecture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadReport {
    strictness: Strictness,
    warnings: Vec<Warning>,
    dropped: u64,
}

/// Inconnues qui deviennent des erreurs en mode strict.
const STRICT_IDS: [&str; 5] = ["U19", "U20", "U21", "U24", "C09"];

impl ReadReport {
    /// Rapport vide pour le mode donné.
    pub fn new(strictness: Strictness) -> Self {
        Self {
            strictness,
            warnings: Vec::new(),
            dropped: 0,
        }
    }

    /// Avertissements conservés, dans l'ordre d'émission.
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Nombre d'avertissements au-delà de la capacité (comptés, non conservés).
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Vrai si au moins un avertissement porte cet identifiant.
    pub fn has(&self, id: &str) -> bool {
        self.warnings.iter().any(|w| w.id == id)
    }

    /// Ajoute les avertissements d'un autre rapport (même capacité maximale).
    pub fn extend(&mut self, other: &ReadReport) {
        for w in &other.warnings {
            if self.warnings.len() < MAX_WARNINGS {
                self.warnings.push(w.clone());
            } else {
                self.dropped = self.dropped.saturating_add(1);
            }
        }
        self.dropped = self.dropped.saturating_add(other.dropped);
    }

    /// Enregistre un avertissement ; en mode strict, erreur pour les cas non vérifiés.
    pub(crate) fn warn(
        &mut self,
        id: &'static str,
        page: Option<u64>,
        detail: &'static str,
    ) -> Result<()> {
        if self.strictness == Strictness::Strict && STRICT_IDS.contains(&id) {
            return Err(Error::Unverified { id });
        }
        if self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(Warning { id, page, detail });
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
        Ok(())
    }
}

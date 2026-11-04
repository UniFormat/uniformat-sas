//! Écriture sur un `Write + Seek` ou dans un fichier, de façon transactionnelle (ADR 0005, ADR 0002).
//!
//! [`SasWriter`] valide le schéma avant le premier octet, écrit les pages de données dès qu'elles sont complètes,
//! puis, à la fin, l'en-tête, la page 0 et la page marqueur (dont le contenu dépend des totaux). Sur un fichier,
//! [`SasWriter::create`] écrit dans un fichier temporaire du même dossier et ne le renomme qu'à
//! [`SasWriter::commit`] : une erreur ou un abandon ne laisse aucun fichier partiel.

use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use uniformat_core::AtomicFile;

use uniformat_sas::write::{
    CellValue, FileBuilder, SchemaError, WriteError, WriteOptions, WriteReport, WriteSchema,
};

/// Secondes entre 1960-01-01 et 1970-01-01.
const SAS_EPOCH_OFFSET: f64 = 315_619_200.0;

/// Erreur d'écriture : entrée/sortie ou données.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteIoError {
    /// Erreur du système de fichiers ou du flux.
    #[error("erreur d'entrée/sortie : {0}")]
    Io(#[from] io::Error),
    /// Schéma invalide ou valeur non représentable.
    #[error(transparent)]
    Write(#[from] WriteError),
}

impl From<SchemaError> for WriteIoError {
    fn from(e: SchemaError) -> Self {
        Self::Write(WriteError::Schema(e))
    }
}

/// Options par défaut avec l'heure du système (date de création) et un code de fichier tiré de l'horloge.
#[must_use]
pub fn system_options() -> WriteOptions {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs_f64();
    // xorshift sur les nanosecondes : le code de fichier n'a pas besoin d'être imprévisible
    let mut x =
        u64::from(now.subsec_nanos()) ^ now.as_secs().rotate_left(32) ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    WriteOptions {
        created: secs + SAS_EPOCH_OFFSET,
        file_code: u32::try_from(x & 0xFFFF_FFFF).unwrap_or(1).max(1),
        ..WriteOptions::default()
    }
}

/// Écrivain de lignes sur un `Write + Seek`.
#[derive(Debug)]
pub struct SasWriter<W: Write + Seek> {
    sink: W,
    builder: Option<FileBuilder>,
}

fn closed() -> WriteIoError {
    WriteIoError::Io(io::Error::other("écrivain déjà terminé"))
}

impl<W: Write + Seek> SasWriter<W> {
    /// Valide le schéma (aucune écriture avant) puis prépare l'écriture sur `sink`.
    ///
    /// # Errors
    /// [`WriteIoError::Write`] : schéma invalide ou limite dépassée.
    pub fn new(sink: W, schema: &WriteSchema, opts: WriteOptions) -> Result<Self, WriteIoError> {
        let builder = FileBuilder::new(schema, opts)?;
        Ok(Self {
            sink,
            builder: Some(builder),
        })
    }

    /// Taille de page utilisée.
    #[must_use]
    pub fn page_length(&self) -> u32 {
        self.builder.as_ref().map_or(0, FileBuilder::page_length)
    }

    /// Écarts comptés jusqu'ici.
    #[must_use]
    pub fn report(&self) -> Option<&WriteReport> {
        self.builder.as_ref().map(FileBuilder::report)
    }

    /// Compteurs modifiables par la couche appelante.
    pub fn report_mut(&mut self) -> Option<&mut WriteReport> {
        self.builder.as_mut().map(FileBuilder::report_mut)
    }

    fn flush_pages(&mut self) -> Result<(), WriteIoError> {
        let Some(b) = self.builder.as_mut() else {
            return Err(closed());
        };
        let p = u64::from(b.page_length());
        for (rank, bytes) in b.take_pages() {
            let off = p.saturating_mul(rank.saturating_add(1));
            self.sink.seek(SeekFrom::Start(off))?;
            self.sink.write_all(&bytes)?;
        }
        Ok(())
    }

    /// Ajoute une ligne.
    ///
    /// # Errors
    /// Valeur non représentable, forme de la ligne ou erreur d'entrée/sortie.
    pub fn write_row(&mut self, cells: &[CellValue<'_>]) -> Result<(), WriteIoError> {
        self.builder.as_mut().ok_or_else(closed)?.push_row(cells)?;
        self.flush_pages()
    }

    /// Termine le fichier : écrit les pages restantes, la page 0, la page marqueur et l'en-tête.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie.
    pub fn finish(mut self) -> Result<(W, WriteReport), WriteIoError> {
        self.flush_pages()?;
        let done = self.builder.take().ok_or_else(closed)?.finish()?;
        let p = u64::from(done.page_length);
        for (rank, bytes) in done.tail_pages.iter().chain(&done.fixed_pages) {
            self.sink
                .seek(SeekFrom::Start(p.saturating_mul(rank.saturating_add(1))))?;
            self.sink.write_all(bytes)?;
        }
        self.sink.seek(SeekFrom::Start(0))?;
        self.sink.write_all(&done.header)?;
        self.sink.flush()?;
        Ok((self.sink, done.report))
    }
}

impl SasWriter<AtomicFile> {
    /// Crée `path` de façon transactionnelle : le schéma est validé avant toute création de fichier, les octets
    /// vont dans un fichier temporaire du même dossier, renommé par [`SasWriter::commit`].
    ///
    /// # Errors
    /// Schéma invalide ou erreur d'entrée/sortie.
    pub fn create(
        path: impl AsRef<Path>,
        schema: &WriteSchema,
        opts: WriteOptions,
    ) -> Result<Self, WriteIoError> {
        let builder = FileBuilder::new(schema, opts)?;
        let sink = AtomicFile::create(path.as_ref())?;
        Ok(Self {
            sink,
            builder: Some(builder),
        })
    }

    /// Termine l'écriture puis renomme le fichier temporaire : le fichier final apparaît d'un coup.
    ///
    /// # Errors
    /// Erreur d'entrée/sortie (le fichier temporaire est alors supprimé).
    pub fn commit(self) -> Result<WriteReport, WriteIoError> {
        let (sink, report) = self.finish()?;
        sink.commit()?;
        Ok(report)
    }
}

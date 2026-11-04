//! `BatchAssembler` : pages décodées → `RecordBatch`, sans I/O.
//!
//! Le même assembleur sert le lecteur synchrone ([`crate::BatchReader`]), le flux asynchrone `object_store` et les
//! partitions DataFusion : l'appelant lui donne des pages dans l'ordre et reprend des lots.

use std::sync::Arc;

use arrow_array::builder::{
    BinaryBuilder, Date32Builder, DurationMicrosecondBuilder, Float64Builder, StringBuilder,
    Time64MicrosecondBuilder, TimestampMicrosecondBuilder, TimestampSecondBuilder, UInt8Builder,
};
use arrow_array::{ArrayRef, RecordBatch, RecordBatchOptions};
use arrow_schema::{Field, Schema, SchemaRef};
use uniformat_sas::temporal::{
    sas_date_to_unix_days, sas_datetime_to_unix_micros, sas_time_to_micros, ConvertError,
};
use uniformat_sas::{
    Column, DecoderState, MissingKind, NumValue, PageDecoder, PageHeader, ReadReport, RowRef,
    TableMeta, TextPolicy, TextValue,
};
use uniformat_sas_io::PagePlan;

use crate::error::{Error, Result};
use crate::options::{OutOfRange, ReadOptions, TrailingBlanks};
use crate::schema::{field_plans, FieldPlan, Repr};

/// Compteurs des écarts rencontrés pendant la lecture (jamais de modification silencieuse, ADR 0003).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReadStats {
    /// Valeurs de date, datetime ou heure hors plage, rendues nulles.
    pub out_of_range: u64,
    /// Champs caractère dont des octets invalides ont été remplacés par U+FFFD.
    pub replaced_text: u64,
    /// Lignes visibles produites.
    pub rows: u64,
}

impl ReadStats {
    /// Cumule `other`.
    pub fn add(&mut self, other: &Self) {
        self.out_of_range = self.out_of_range.saturating_add(other.out_of_range);
        self.replaced_text = self.replaced_text.saturating_add(other.replaced_text);
        self.rows = self.rows.saturating_add(other.rows);
    }
}

enum Builder {
    Float64(Float64Builder),
    Date32(Date32Builder),
    TimestampMicro(TimestampMicrosecondBuilder),
    TimestampSecond(TimestampSecondBuilder),
    Time64Micro(Time64MicrosecondBuilder),
    DurationMicro(DurationMicrosecondBuilder),
    Utf8(StringBuilder),
    Binary(BinaryBuilder),
    Sidecar(UInt8Builder),
}

impl Builder {
    fn new(repr: Repr, capacity: usize) -> Self {
        match repr {
            Repr::Float64 => Self::Float64(Float64Builder::with_capacity(capacity)),
            Repr::Date32 => Self::Date32(Date32Builder::with_capacity(capacity)),
            Repr::TimestampMicro => {
                Self::TimestampMicro(TimestampMicrosecondBuilder::with_capacity(capacity))
            }
            Repr::TimestampSecond => {
                Self::TimestampSecond(TimestampSecondBuilder::with_capacity(capacity))
            }
            Repr::Time64Micro => {
                Self::Time64Micro(Time64MicrosecondBuilder::with_capacity(capacity))
            }
            Repr::DurationMicro => {
                Self::DurationMicro(DurationMicrosecondBuilder::with_capacity(capacity))
            }
            Repr::Utf8 => Self::Utf8(StringBuilder::with_capacity(
                capacity,
                capacity.saturating_mul(8),
            )),
            Repr::Binary => Self::Binary(BinaryBuilder::with_capacity(
                capacity,
                capacity.saturating_mul(8),
            )),
            Repr::SidecarU8 => Self::Sidecar(UInt8Builder::with_capacity(capacity)),
        }
    }

    fn finish(&mut self) -> ArrayRef {
        match self {
            Self::Float64(b) => Arc::new(b.finish()),
            Self::Date32(b) => Arc::new(b.finish()),
            Self::TimestampMicro(b) => Arc::new(b.finish()),
            Self::TimestampSecond(b) => Arc::new(b.finish()),
            Self::Time64Micro(b) => Arc::new(b.finish()),
            Self::DurationMicro(b) => Arc::new(b.finish()),
            Self::Utf8(b) => Arc::new(b.finish()),
            Self::Binary(b) => Arc::new(b.finish()),
            Self::Sidecar(b) => Arc::new(b.finish()),
        }
    }
}

/// Une colonne de sortie : la colonne SAS, son rôle et son constructeur.
struct OutColumn {
    source: usize,
    builder: Builder,
}

/// Code de la colonne annexe (ADR 0003) : `0` présente, `1` `.`, `2` `._`, `3 + (lettre − 'A')`.
fn sidecar_code(v: NumValue) -> u8 {
    match v {
        NumValue::Value(_) => 0,
        NumValue::Missing(MissingKind::Dot) => 1,
        NumValue::Missing(MissingKind::Underscore) => 2,
        NumValue::Missing(MissingKind::Letter(c)) => 3u8.saturating_add(c.saturating_sub(b'A')),
    }
}

/// Valeur d'une conversion temporelle : `Ok(Some)`, `Ok(None)` (nul compté) ou l'erreur à rendre.
enum Converted<T> {
    Value(T),
    Null,
}

fn convert<T>(
    r: std::result::Result<T, ConvertError>,
    policy: OutOfRange,
    stats: &mut ReadStats,
    column: usize,
    row: u64,
    error: &mut Option<Error>,
) -> Converted<T> {
    match r {
        Ok(v) => Converted::Value(v),
        Err(_) => {
            stats.out_of_range = stats.out_of_range.saturating_add(1);
            if policy == OutOfRange::Error && error.is_none() {
                *error = Some(Error::OutOfRange { column, row });
            }
            Converted::Null
        }
    }
}

/// Assembleur de lots pour une projection de colonnes.
pub struct BatchAssembler {
    meta: Arc<TableMeta>,
    state: Option<DecoderState>,
    schema: SchemaRef,
    columns: Vec<OutColumn>,
    opts: ReadOptions,
    policy: TextPolicy,
    rows_in_batch: usize,
    remaining: Option<u64>,
    stats: ReadStats,
    error: Option<Error>,
}

impl std::fmt::Debug for BatchAssembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchAssembler")
            .field("schema", &self.schema)
            .field("rows_in_batch", &self.rows_in_batch)
            .field("remaining", &self.remaining)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl BatchAssembler {
    /// Assembleur pour `projection` (indices dans le schéma complet de [`crate::arrow_schema`], `None` = tout),
    /// qui s'arrête après `limit` lignes visibles. `rows_before` : lignes physiques des pages précédentes (0 pour
    /// un fichier lu depuis le début ; une table compressée se lit toujours depuis la première page).
    ///
    /// # Errors
    /// [`Error::Projection`] si un indice dépasse le schéma.
    pub fn new(
        meta: Arc<TableMeta>,
        opts: &ReadOptions,
        projection: Option<&[usize]>,
        limit: Option<u64>,
        rows_before: u64,
    ) -> Result<Self> {
        let plans = field_plans(&meta, opts);
        let chosen: Vec<FieldPlan> = match projection {
            None => plans,
            Some(p) => p
                .iter()
                .map(|&i| plans.get(i).cloned().ok_or(Error::Projection(i)))
                .collect::<Result<_>>()?,
        };
        let schema_fields: Vec<Field> = chosen.iter().map(|p| p.field.clone()).collect();
        let mut schema_meta = std::collections::HashMap::new();
        schema_meta.insert("sas.table_name".to_owned(), meta.header.table_name.clone());
        let schema = Arc::new(Schema::new_with_metadata(schema_fields, schema_meta));
        // Préalloue au plus ce que le lecteur produira : une table large de peu de lignes ne réserve pas
        // `batch_rows` valeurs par colonne.
        let rows_hint = usize::try_from(meta.row_count).unwrap_or(usize::MAX);
        let limit_hint = limit.map_or(usize::MAX, |l| usize::try_from(l).unwrap_or(usize::MAX));
        let capacity = opts
            .batch_rows
            .min(1 << 20)
            .min(rows_hint)
            .min(limit_hint)
            .max(1);
        let columns = chosen
            .iter()
            .map(|p| OutColumn {
                source: p.source,
                builder: Builder::new(p.repr, capacity),
            })
            .collect();
        let policy = TextPolicy {
            invalid: opts.invalid_bytes,
            trim_trailing: opts.trailing_blanks == TrailingBlanks::Trim,
        };
        let state = DecoderState::new(&meta, opts.strictness, opts.encoding_override)
            .starting_at(rows_before);
        Ok(Self {
            meta,
            state: Some(state),
            schema,
            columns,
            opts: opts.clone(),
            policy,
            rows_in_batch: 0,
            remaining: limit,
            stats: ReadStats::default(),
            error: None,
        })
    }

    /// Schéma des lots produits (projection appliquée).
    #[must_use]
    pub fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    /// Taille d'une page de la table.
    #[must_use]
    pub fn page_length(&self) -> u32 {
        self.meta.header.page_length
    }

    /// Écarts comptés depuis la création.
    #[must_use]
    pub fn stats(&self) -> &ReadStats {
        &self.stats
    }

    /// Avertissements du décodage (ADR 0006).
    #[must_use]
    pub fn report(&self) -> Option<&ReadReport> {
        self.state.as_ref().map(DecoderState::report)
    }

    /// La limite de lignes est atteinte : inutile de fournir d'autres pages.
    #[must_use]
    pub fn limit_reached(&self) -> bool {
        self.remaining == Some(0)
    }

    /// Le lot en cours a atteint `batch_rows` lignes.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.rows_in_batch >= self.opts.batch_rows.max(1)
    }

    /// Lignes en attente dans le lot en cours.
    #[must_use]
    pub fn pending_rows(&self) -> usize {
        self.rows_in_batch
    }

    /// Décode les lignes visibles de la page `page_index` (`page` : exactement une page). Si `plan` est donné, le
    /// nombre de lignes physiques de la page est vérifié contre le plan rapide.
    ///
    /// # Errors
    /// Erreur du cœur (page, flot compressé), `PlanMismatch`, ou [`Error::OutOfRange`] / `Encoding` selon les
    /// options.
    pub fn push_page(
        &mut self,
        page_index: u64,
        page: &[u8],
        plan: Option<&PagePlan>,
    ) -> Result<()> {
        if self.limit_reached() {
            return Ok(());
        }
        if let Some(plan) = plan {
            let mut scratch = ReadReport::new(uniformat_sas::Strictness::Lenient);
            let ph = PageHeader::parse_prefix(&self.meta.header, page_index, page, &mut scratch)?;
            plan.check_page(page_index, ph.physical_rows()?)?;
        }
        let state = self
            .state
            .take()
            .ok_or(Error::Unsupported("assembleur en erreur"))?;
        let mut decoder = PageDecoder::resume(&self.meta, state);
        let meta = &self.meta;
        let columns = &mut self.columns;
        let (opts, policy) = (&self.opts, &self.policy);
        let (stats, error) = (&mut self.stats, &mut self.error);
        let (remaining, in_batch) = (&mut self.remaining, &mut self.rows_in_batch);
        let result = decoder.visit_rows(page_index, page, |row| {
            if error.is_some() || *remaining == Some(0) {
                return;
            }
            for c in columns.iter_mut() {
                if let Some(col) = meta.columns.get(c.source) {
                    append(c, col, &row, opts, policy, stats, error);
                }
            }
            if error.is_some() {
                return;
            }
            *in_batch = in_batch.saturating_add(1);
            stats.rows = stats.rows.saturating_add(1);
            if let Some(r) = remaining.as_mut() {
                *r = r.saturating_sub(1);
            }
        });
        self.state = Some(decoder.into_state());
        result.map_err(Error::from)?;
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Décode les pages `first_page..` contenues dans `bytes` (un nombre entier de pages de `page_length`
    /// octets), en s'arrêtant dès que la limite est atteinte.
    ///
    /// # Errors
    /// Mêmes erreurs que [`BatchAssembler::push_page`] ; `Truncated` si `bytes` n'est pas un nombre entier de pages.
    pub fn push_pages(
        &mut self,
        first_page: u64,
        bytes: &[u8],
        plan: Option<&PagePlan>,
    ) -> Result<()> {
        let plen = usize::try_from(self.meta.header.page_length).unwrap_or(usize::MAX);
        if plen == 0 || !bytes.len().is_multiple_of(plen) {
            return Err(uniformat_sas::Error::Truncated {
                needed: plen as u64,
                got: bytes.len() as u64,
            }
            .into());
        }
        for (i, page) in bytes.chunks_exact(plen).enumerate() {
            if self.limit_reached() {
                break;
            }
            self.push_page(first_page.saturating_add(i as u64), page, plan)?;
        }
        Ok(())
    }

    /// Termine le lot en cours (`None` s'il est vide, sauf projection vide qui garde le compte de lignes).
    ///
    /// # Errors
    /// [`Error::Arrow`] si les tableaux sont incohérents (ne devrait pas arriver).
    pub fn take_batch(&mut self) -> Result<Option<RecordBatch>> {
        if self.rows_in_batch == 0 {
            return Ok(None);
        }
        let rows = self.rows_in_batch;
        self.rows_in_batch = 0;
        let arrays: Vec<ArrayRef> = self
            .columns
            .iter_mut()
            .map(|c| c.builder.finish())
            .collect();
        let batch = RecordBatch::try_new_with_options(
            Arc::clone(&self.schema),
            arrays,
            &RecordBatchOptions::new().with_row_count(Some(rows)),
        )?;
        Ok(Some(batch))
    }
}

/// Ajoute la valeur de `row` pour la colonne de sortie `c`.
fn append(
    c: &mut OutColumn,
    col: &Column,
    row: &RowRef<'_>,
    opts: &ReadOptions,
    policy: &TextPolicy,
    stats: &mut ReadStats,
    error: &mut Option<Error>,
) {
    let at = row.physical_index();
    match &mut c.builder {
        Builder::Sidecar(b) => b.append_value(sidecar_code(row.num(col))),
        Builder::Float64(b) => match row.num(col) {
            NumValue::Value(v) => b.append_value(v),
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::Date32(b) => match row.num(col) {
            NumValue::Value(v) => match convert(
                sas_date_to_unix_days(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(d) => b.append_value(d),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::TimestampMicro(b) => match row.num(col) {
            NumValue::Value(v) => match convert(
                sas_datetime_to_unix_micros(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(t) => b.append_value(t),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::TimestampSecond(b) => match row.num(col) {
            NumValue::Value(v) => match convert(
                sas_datetime_to_unix_micros(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(t) => b.append_value(t.div_euclid(1_000_000)),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::Time64Micro(b) => match row.num(col) {
            NumValue::Value(v) => {
                // Time64 ne représente que 0 ≤ t < 86 400 s
                let r = sas_time_to_micros(v).and_then(|t| {
                    if (0..86_400_000_000).contains(&t) {
                        Ok(t)
                    } else {
                        Err(ConvertError::OutOfRange)
                    }
                });
                match convert(r, opts.out_of_range, stats, col.index, at, error) {
                    Converted::Value(t) => b.append_value(t),
                    Converted::Null => b.append_null(),
                }
            }
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::DurationMicro(b) => match row.num(col) {
            NumValue::Value(v) => match convert(
                sas_time_to_micros(v),
                opts.out_of_range,
                stats,
                col.index,
                at,
                error,
            ) {
                Converted::Value(t) => b.append_value(t),
                Converted::Null => b.append_null(),
            },
            NumValue::Missing(_) => b.append_null(),
        },
        Builder::Utf8(b) => match row.text_value(col, policy) {
            Ok(TextValue::Text { value, replaced }) => {
                if replaced {
                    stats.replaced_text = stats.replaced_text.saturating_add(1);
                }
                if opts.empty_as_null && value.is_empty() {
                    b.append_null();
                } else {
                    b.append_value(&value);
                }
            }
            Ok(TextValue::Bytes(_)) => b.append_null(),
            Err(e) => {
                if error.is_none() {
                    *error = Some(e.into());
                }
                b.append_null();
            }
        },
        Builder::Binary(b) => {
            let raw = if policy.trim_trailing {
                row.char_bytes(col)
            } else {
                row.bytes(col)
            };
            if opts.empty_as_null && raw.is_empty() {
                b.append_null();
            } else {
                b.append_value(raw);
            }
        }
    }
}

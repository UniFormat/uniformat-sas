//! Conversion d'une table SAS en colonnes R (`data.frame`), sans dépendance à R (ADR 0003, « Restitution R »).
//!
//! - numériques : `double` ; `.` devient `NA_real_`, `._` et `.A` à `.Z` deviennent des **NA étiquetés** au motif de
//!   `haven` ([`tagged_na`], [`na_tag`]) ; `.A` à `.Z` portent les étiquettes minuscules `a` à `z`, comme `haven::read_sas` ;
//! - dates : classe `Date` (jours depuis 1970), date-heures : `POSIXct` UTC (secondes depuis 1970), heures :
//!   `hms::hms` (secondes) ;
//! - attributs de colonne : `label`, `format.sas`, `informat.sas`, `width` ; attributs de table : `label`,
//!   `sas7bdat.created`, `sas7bdat.encoding`, `sas7bdat.compression`.

use crate::read_budget::ReadBudget;
use std::borrow::Cow;
use std::path::Path;

use uniformat_sas::{
    classify_format, Column, ColumnKind, Compression, Encoding, LogicalType, MissingKind, NumValue,
    SasFormat, StorageKind, Strictness, TextPolicy, TextValue,
};
use uniformat_sas_io::{FileSource, OpenOptions, SasFile};

/// Secondes entre 1960-01-01 et 1970-01-01.
const SAS_TO_UNIX_SECONDS: f64 = 315_619_200.0;
/// Jours entre 1960-01-01 et 1970-01-01.
const SAS_TO_UNIX_DAYS: f64 = 3_653.0;
/// Mot de poids faible du `NA_real_` de R (1954).
const R_NA_LOW_WORD: u64 = 1954;

/// `NA_real_` de R : un NaN dont le mot de poids faible vaut 1954.
#[must_use]
pub fn na_real() -> f64 {
    f64::from_bits(0x7FF0_0000_0000_0000 | R_NA_LOW_WORD)
}

/// NA étiqueté de `haven` : `NA_real_` dont l'octet de rang 4 (little endian) porte le code de l'étiquette.
#[must_use]
pub fn tagged_na(tag: u8) -> f64 {
    f64::from_bits(0x7FF0_0000_0000_0000 | (u64::from(tag) << 32) | R_NA_LOW_WORD)
}

/// Étiquette d'un NA étiqueté (`None` pour une valeur ordinaire ou un `NA_real_` sans étiquette).
#[must_use]
pub fn na_tag(x: f64) -> Option<u8> {
    let bits = x.to_bits();
    if !x.is_nan() || bits & 0xFFFF_FFFF != R_NA_LOW_WORD {
        return None;
    }
    let tag = u8::try_from((bits >> 32) & 0xFF).unwrap_or(0);
    (tag != 0).then_some(tag)
}

/// Classe R d'une colonne numérique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RClass {
    /// `numeric`.
    Plain,
    /// `Date`.
    Date,
    /// `POSIXct` (UTC).
    DateTime,
    /// `hms::hms`.
    Time,
}

/// Données d'une colonne.
#[derive(Debug, Clone, PartialEq)]
pub enum RData {
    /// `double`.
    Real(Vec<f64>),
    /// `character` ; `None` = `NA_character_`.
    Str(Vec<Option<String>>),
}

/// Une colonne de `data.frame` et ses attributs SAS.
#[derive(Debug, Clone, PartialEq)]
pub struct RColumn {
    /// Nom.
    pub name: String,
    /// Valeurs.
    pub data: RData,
    /// Classe R.
    pub class: RClass,
    /// Attribut `label`.
    pub label: Option<String>,
    /// Attribut `format.sas`.
    pub format_sas: Option<String>,
    /// Attribut `informat.sas`.
    pub informat_sas: Option<String>,
    /// Attribut `width` (largeur de stockage SAS).
    pub width: Option<u32>,
}

/// Table lue.
#[derive(Debug, Clone, PartialEq)]
pub struct RTable {
    /// Colonnes, dans l'ordre du fichier (ou de `col_select`).
    pub columns: Vec<RColumn>,
    /// Nombre de lignes.
    pub rows: usize,
    /// Attributs de table : `label`, `sas7bdat.created`, `sas7bdat.encoding`, `sas7bdat.compression`.
    pub attrs: Vec<(String, String)>,
    /// Avertissements du décodage (`id : détail`).
    pub warnings: Vec<String>,
}

/// Options de lecture (`read_sas7bdat`).
#[derive(Debug, Clone)]
pub struct RReadOptions {
    /// Colonnes à garder (noms), dans l'ordre demandé.
    pub col_select: Option<Vec<String>>,
    /// Nombre maximal de lignes.
    pub n_max: Option<usize>,
    /// Encodage imposé.
    pub encoding: Option<String>,
    /// Retirer les blancs de fin des chaînes (défaut).
    pub trim_ws: bool,
    /// Les cas non vérifiés par la spécification sont des erreurs.
    pub strict: bool,
    /// Budget conservateur des colonnes de sortie, en octets (hors interpréteur R).
    pub max_bytes: usize,
}

impl Default for RReadOptions {
    fn default() -> Self {
        Self {
            col_select: None,
            n_max: None,
            encoding: None,
            trim_ws: true,
            strict: false,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Erreur de lecture pour R : condition `sas7bdat_error` (ADR 0007).
#[derive(Debug)]
pub enum RError {
    /// Entrée/sortie.
    Io(String),
    /// Fichier invalide.
    Format(String),
    /// Option invalide.
    Usage(String),
    /// Limite mémoire dépassée ou réservation impossible.
    ResourceLimit(String),
}

impl std::fmt::Display for RError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(m) | Self::Format(m) | Self::Usage(m) | Self::ResourceLimit(m) => {
                f.write_str(m)
            }
        }
    }
}

impl std::error::Error for RError {}

impl RError {
    /// Classe de la condition R (`sas7bdat_io_error`, `sas7bdat_format_error`, `sas7bdat_usage_error`).
    #[must_use]
    pub fn class(&self) -> &'static str {
        match self {
            Self::Io(_) => "sas7bdat_io_error",
            Self::Format(_) => "sas7bdat_format_error",
            Self::Usage(_) => "sas7bdat_usage_error",
            Self::ResourceLimit(_) => "sas7bdat_resource_error",
        }
    }
}

/// Une chaîne R ne peut pas contenir de NUL : coupe au premier NUL (sémantique des chaînes C). Rend vrai si coupée.
fn cut_nul(s: &mut String) -> bool {
    match s.find('\0') {
        Some(i) => {
            s.truncate(i);
            true
        }
        None => false,
    }
}

fn format_name(f: &SasFormat) -> String {
    let mut s = f.name.clone();
    if f.width > 0 {
        s.push_str(&f.width.to_string());
    }
    if f.decimals > 0 {
        s.push('.');
        s.push_str(&f.decimals.to_string());
    }
    s
}

/// Classe R d'une colonne numérique : d'après son **format** seulement (comme `haven` et l'affichage de SAS) ;
/// un informat seul ne change pas la classe.
fn class_of(col: &Column) -> RClass {
    if !matches!(col.kind, ColumnKind::Numeric { .. }) {
        return RClass::Plain;
    }
    let Some(f) = &col.format else {
        return RClass::Plain;
    };
    match classify_format(&f.name, StorageKind::Numeric) {
        LogicalType::Date => RClass::Date,
        LogicalType::DateTime => RClass::DateTime,
        LogicalType::Time => RClass::Time,
        _ => RClass::Plain,
    }
}

fn convert_num(class: RClass, v: NumValue) -> f64 {
    match v {
        NumValue::Missing(MissingKind::Dot) => na_real(),
        NumValue::Missing(MissingKind::Underscore) => tagged_na(b'_'),
        // `haven::read_sas` (ReadStat) rend `.A` à `.Z` avec les étiquettes minuscules `a` à `z`
        NumValue::Missing(MissingKind::Letter(c)) => tagged_na(c.to_ascii_lowercase()),
        NumValue::Value(x) => match class {
            RClass::Date => x - SAS_TO_UNIX_DAYS,
            RClass::DateTime => x - SAS_TO_UNIX_SECONDS,
            RClass::Plain | RClass::Time => x,
        },
    }
}

fn iso_created(h: &uniformat_sas::FileHeader) -> Option<String> {
    // création en heure locale de l'écrivain → UTC (décalage de l'en-tête)
    let utc = h.created - h.created_tz_offset;
    let unix = utc - SAS_TO_UNIX_SECONDS;
    if !unix.is_finite() {
        return None;
    }
    let secs = unix.floor();
    let days = (secs / 86_400.0).floor();
    let rem = secs - days * 86_400.0;
    let (y, m, d) = civil_from_days(days as i64);
    let (hh, mm, ss) = (
        (rem / 3600.0) as i64,
        ((rem % 3600.0) / 60.0) as i64,
        (rem % 60.0) as i64,
    );
    Some(format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"))
}

/// Date civile (calendrier grégorien proleptique) depuis un nombre de jours depuis 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Lit un fichier `.sas7bdat` en colonnes R.
///
/// # Errors
/// Entrée/sortie, fichier invalide, option invalide.
pub fn read_table(path: &Path, opts: &RReadOptions) -> Result<RTable, RError> {
    let mut open = OpenOptions::default();
    if let Some(label) = &opts.encoding {
        open.encoding_override = Some(
            Encoding::for_label(label.as_bytes())
                .ok_or_else(|| RError::Usage(format!("encoding {label:?} inconnu")))?,
        );
    }
    open.strictness = if opts.strict {
        Strictness::Strict
    } else {
        Strictness::Lenient
    };
    let source =
        FileSource::open(path).map_err(|e| RError::Io(format!("{} : {e}", path.display())))?;
    let file = SasFile::open(source, open).map_err(|e| match e {
        uniformat_sas_io::IoError::Io(io) => RError::Io(io.to_string()),
        other => RError::Format(other.to_string()),
    })?;
    let meta = file.meta();
    let selected: Vec<&Column> =
        match &opts.col_select {
            None => meta.columns.iter().collect(),
            Some(names) => {
                let mut out = Vec::with_capacity(names.len());
                for n in names {
                    out.push(meta.columns.iter().find(|c| &c.name == n).ok_or_else(|| {
                        RError::Usage(format!("colonne {n:?} absente de la table"))
                    })?);
                }
                out
            }
        };
    let budget = ReadBudget::new(&selected, meta.visible_rows(), opts)?;
    // Never allocate from an untrusted declared row count. Grow only as rows are decoded.
    let mut data: Vec<RData> = selected
        .iter()
        .map(|c| match c.kind {
            ColumnKind::Numeric { .. } => RData::Real(Vec::new()),
            ColumnKind::Char { .. } => RData::Str(Vec::new()),
        })
        .collect();
    let classes: Vec<RClass> = selected.iter().map(|c| class_of(c)).collect();
    let policy = TextPolicy {
        trim_trailing: opts.trim_ws,
        ..TextPolicy::default()
    };
    let limit = opts.n_max.unwrap_or(usize::MAX);
    let mut rows = 0usize;
    let mut replaced = 0u64;
    let mut nul_cut = 0u64;
    let mut failure: Option<RError> = None;
    let mut decoder = file.decoder();
    let page_count = meta.header.page_count;
    'pages: for k in 0..page_count {
        if rows >= limit {
            break;
        }
        let bytes = file
            .read_pages(&(k..k + 1))
            .map_err(|e| RError::Format(e.to_string()))?;
        decoder
            .visit_rows(k, &bytes, |row| {
                if rows >= limit || failure.is_some() {
                    return;
                }
                if let Err(e) = budget.reserve_row(rows, &mut data) {
                    failure = Some(e);
                    return;
                }
                for (j, col) in selected.iter().enumerate() {
                    match data.get_mut(j) {
                        Some(RData::Real(v)) => {
                            v.push(convert_num(classes[j], row.num(col)));
                        }
                        Some(RData::Str(v)) => match row.text_value(col, &policy) {
                            Ok(TextValue::Text { value, replaced: r }) => {
                                replaced += u64::from(r);
                                let mut text = value.into_owned();
                                nul_cut += u64::from(cut_nul(&mut text));
                                v.push(Some(text));
                            }
                            Ok(TextValue::Bytes(b)) => {
                                replaced += 1;
                                let mut text = String::from_utf8_lossy(b).into_owned();
                                nul_cut += u64::from(cut_nul(&mut text));
                                v.push(Some(text));
                            }
                            Err(e) => {
                                failure = Some(RError::Format(e.to_string()));
                                return;
                            }
                        },
                        None => {}
                    }
                }
                rows += 1;
            })
            .map_err(|e| RError::Format(e.to_string()))?;
        if failure.is_some() {
            break 'pages;
        }
    }
    if let Some(e) = failure {
        return Err(e);
    }
    let mut warnings: Vec<String> = file
        .report()
        .warnings()
        .iter()
        .chain(decoder.report().warnings())
        .map(|w| format!("{} : {}", w.id, w.detail))
        .collect();
    if replaced > 0 {
        warnings.push(format!(
            "{replaced} champs texte contiennent des octets invalides pour l'encodage, remplacés"
        ));
    }
    if nul_cut > 0 {
        warnings.push(format!(
            "{nul_cut} champs texte contiennent un octet NUL (non représentable en R) : coupés au premier NUL"
        ));
    }
    let clean = |s: String| {
        let mut s = s;
        cut_nul(&mut s);
        s
    };
    let columns = selected
        .iter()
        .zip(data)
        .zip(classes)
        .map(|((c, data), class)| RColumn {
            name: clean(c.name.clone()),
            data,
            class,
            label: c.label.clone().map(clean).filter(|l| !l.is_empty()),
            format_sas: c
                .format
                .as_ref()
                .map(format_name)
                .map(clean)
                .filter(|f| !f.is_empty()),
            informat_sas: c
                .informat
                .as_ref()
                .map(format_name)
                .map(clean)
                .filter(|f| !f.is_empty()),
            width: Some(u32::try_from(c.width()).unwrap_or(u32::MAX)),
        })
        .collect();
    let h = &meta.header;
    let mut attrs = Vec::new();
    if !h.table_name.is_empty() {
        attrs.push(("table_name".to_owned(), clean(h.table_name.clone())));
    }
    if let Some(created) = iso_created(h) {
        attrs.push(("sas7bdat.created".to_owned(), created));
    }
    let encoding = uniformat_sas::encoding_info(h.encoding_code).map_or_else(
        || Cow::Owned(h.encoding_code.to_string()),
        |i| Cow::Borrowed(i.sas_name),
    );
    attrs.push(("sas7bdat.encoding".to_owned(), encoding.into_owned()));
    attrs.push((
        "sas7bdat.deleted_rows".to_owned(),
        meta.deleted_count.to_string(),
    ));
    attrs.push((
        "sas7bdat.compression".to_owned(),
        match meta.compression {
            Compression::None => "none",
            Compression::Char => "char",
            Compression::Binary => "binary",
        }
        .to_owned(),
    ));
    Ok(RTable {
        columns,
        rows,
        attrs,
        warnings,
    })
}

/// Colonne R à écrire (déjà normalisée côté R : `double` ou `character`).
#[derive(Debug, Clone, PartialEq)]
pub struct RWriteColumn {
    /// Nom.
    pub name: String,
    /// Valeurs.
    pub data: RData,
    /// Classe R d'origine (dates, date-heures, heures : valeurs déjà en jours/secondes depuis 1970).
    pub class: RClass,
    /// Label.
    pub label: Option<String>,
    /// Format SAS (`DATE9`, `COMMA10.2`, `$CHAR20`).
    pub format_sas: Option<String>,
    /// Informat SAS.
    pub informat_sas: Option<String>,
}

/// Options d'écriture (`write_sas7bdat`).
#[derive(Debug, Clone)]
pub struct RWriteOptions {
    /// `none`, `char` ou `binary`.
    pub compression: String,
    /// `UTF-8`, `latin1` ou `wlatin1`.
    pub encoding: String,
    /// Taille de page demandée.
    pub page_length: u32,
    /// `error`, `replace` ou `truncate`.
    pub on_unrepresentable: String,
}

impl Default for RWriteOptions {
    fn default() -> Self {
        Self {
            compression: "none".to_owned(),
            encoding: "UTF-8".to_owned(),
            page_length: 65_536,
            on_unrepresentable: "error".to_owned(),
        }
    }
}

/// Décompose `DATE9.`, `COMMA10.2`, `$CHAR20.` en (nom, largeur, décimales).
fn parse_format(spec: &str) -> Option<(String, u16, u16)> {
    let t = spec.trim().trim_end_matches('.');
    if t.is_empty() {
        return None;
    }
    let (head, dec) = match t.split_once('.') {
        Some((h, d)) => (h, d.parse::<u16>().unwrap_or(0)),
        None => (t, 0),
    };
    let digits = head.chars().rev().take_while(char::is_ascii_digit).count();
    let split = head.len().saturating_sub(digits);
    let (name, width) = head.split_at(split);
    Some((name.to_owned(), width.parse::<u16>().unwrap_or(0), dec))
}

/// Manquant SAS d'un NaN de R : `NA_real_` → `.`, NA étiqueté → `_` ou `.A` à `.Z` (étiquettes `haven`).
fn missing_of(x: f64) -> MissingKind {
    match na_tag(x) {
        Some(b'_') => MissingKind::Underscore,
        Some(t) if t.is_ascii_alphabetic() => MissingKind::Letter(t.to_ascii_uppercase()),
        _ => MissingKind::Dot,
    }
}

/// Écrit `columns` dans `path` (transactionnel). Rend le nombre de lignes et les avertissements.
///
/// # Errors
/// Option invalide, schéma ou valeur non représentable, entrée/sortie.
pub fn write_table(
    path: &Path,
    columns: &[RWriteColumn],
    opts: &RWriteOptions,
) -> Result<(u64, Vec<String>), RError> {
    use uniformat_sas::write::{
        CellValue, Unrepresentable, WriteColumn, WriteEncoding, WriteOptions, WriteSchema,
    };
    use uniformat_sas_io::{system_options, SasWriter};
    let encoding = match opts.encoding.to_ascii_lowercase().as_str() {
        "utf-8" | "utf8" => WriteEncoding::Utf8,
        "latin1" | "latin-1" | "iso-8859-1" => WriteEncoding::Latin1,
        "wlatin1" | "cp1252" | "windows-1252" => WriteEncoding::Wlatin1,
        other => {
            return Err(RError::Usage(format!(
                "encoding {other:?} non pris en charge"
            )))
        }
    };
    let compression = match opts.compression.as_str() {
        "none" => Compression::None,
        "char" => Compression::Char,
        "binary" => Compression::Binary,
        other => return Err(RError::Usage(format!("compression {other:?} inconnue"))),
    };
    let policy = match opts.on_unrepresentable.as_str() {
        "error" => Unrepresentable::Error,
        "replace" => Unrepresentable::Replace,
        "truncate" => Unrepresentable::Truncate,
        other => {
            return Err(RError::Usage(format!(
                "on_unrepresentable {other:?} inconnu"
            )))
        }
    };
    let rows = columns.first().map_or(0, |c| match &c.data {
        RData::Real(v) => v.len(),
        RData::Str(v) => v.len(),
    });
    let mut schema = WriteSchema::default();
    for c in columns {
        let mut col = match &c.data {
            RData::Real(_) => WriteColumn::numeric(c.name.clone(), 8),
            RData::Str(v) => {
                let len = v
                    .iter()
                    .flatten()
                    .map(|s| encoding.encode_lossy(s).0.len())
                    .max()
                    .unwrap_or(1)
                    .max(1);
                WriteColumn::character(c.name.clone(), u32::try_from(len).unwrap_or(u32::MAX))
            }
        };
        if let Some(l) = &c.label {
            col.label.clone_from(l);
        }
        let default_format = match c.class {
            RClass::Date => Some(("DATE".to_owned(), 9, 0)),
            RClass::DateTime => Some(("DATETIME".to_owned(), 20, 0)),
            RClass::Time => Some(("TIME".to_owned(), 8, 0)),
            RClass::Plain => None,
        };
        if let Some((n, w, d)) = c
            .format_sas
            .as_deref()
            .and_then(parse_format)
            .or(default_format)
        {
            col = col.with_format(n, w, d);
        }
        if let Some((n, w, d)) = c.informat_sas.as_deref().and_then(parse_format) {
            col = col.with_informat(n, w, d);
        }
        schema.columns.push(col);
    }
    let wopts = WriteOptions {
        encoding,
        compression,
        page_length: opts.page_length,
        on_unrepresentable: policy,
        ..system_options()
    };
    let io_err = |e: uniformat_sas_io::WriteIoError| match e {
        uniformat_sas_io::WriteIoError::Io(io) => RError::Io(io.to_string()),
        other => RError::Format(other.to_string()),
    };
    let mut w = SasWriter::create(path, &schema, wopts).map_err(io_err)?;
    let mut cells: Vec<CellValue<'_>> = Vec::with_capacity(columns.len());
    for i in 0..rows {
        cells.clear();
        for c in columns {
            cells.push(match &c.data {
                RData::Real(v) => match v.get(i).copied() {
                    Some(x) if x.is_nan() => CellValue::Missing(missing_of(x)),
                    Some(x) => CellValue::Num(match c.class {
                        RClass::Date => x + SAS_TO_UNIX_DAYS,
                        RClass::DateTime => x + SAS_TO_UNIX_SECONDS,
                        _ => x,
                    }),
                    None => CellValue::Null,
                },
                RData::Str(v) => match v.get(i) {
                    Some(Some(s)) => CellValue::Text(s),
                    _ => CellValue::Null,
                },
            });
        }
        w.write_row(&cells).map_err(io_err)?;
    }
    let report = w.commit().map_err(io_err)?;
    Ok((report.rows, report.warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn na_patterns_roundtrip() {
        assert!(na_real().is_nan());
        assert_eq!(na_tag(na_real()), None);
        for &tag in b"AZ_" {
            let x = tagged_na(tag);
            assert!(x.is_nan());
            assert_eq!(na_tag(x), Some(tag));
        }
        assert_eq!(na_tag(1.5), None);
        assert_eq!(na_tag(f64::NAN), None);
    }

    #[test]
    fn na_bits_follow_r_layout() {
        // NA_real_ de R : 0x7FF00000000007A2 ; l'étiquette est l'octet 4 en little endian
        assert_eq!(na_real().to_bits(), 0x7FF0_0000_0000_07A2);
        assert_eq!(tagged_na(b'A').to_bits(), 0x7FF0_0041_0000_07A2);
        assert_eq!(tagged_na(b'A').to_le_bytes()[4], b'A');
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-3653), (1960, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }
}

# Architecture du driver Rust `sas7bdat`

Conception du 2026-10-30 (changement `design-rust-driver`, prompt 06). Documents en français ; identifiants, signatures et pseudo-code en anglais. **Aucune implémentation n'est écrite** : les blocs `rust` ci-dessous sont des signatures (corps absents), pas du code compilable.

## 0. Statut et réserves

- **La spécification amont est partielle.** `rea_output/STATE.md` marque les prompts 02, 03, 04 et 05 `partiel` (décompilation non faite, aucune entrée `confirmé`) et la confirmation interne NULL n'y est pas consignée. La tâche 1.1 du changement demandait de s'arrêter dans ce cas ; la conception a été poursuivie **sur instruction explicite de l'utilisateur** (« Oui, lance 06 ») après avoir été informé de l'état de 05. Conséquence : toute règle de la spécification est `inféré` ou `inconnu`, et chaque point `inféré` reçoit un comportement défensif (ADR 0006). Le blocage reste ouvert dans `STATE.md`.
- **Séparation analyse / conception.** Cette session n'est pas une session neuve : elle continue la conversation qui a produit les spécifications 02 à 05. La séparation repose donc sur la discipline de lecture (liste ci-dessous) et non sur l'ignorance de l'auteur. Aucune information tirée de `src/`, de `rea_output/evidence/` ou d'une sortie de décompilation n'est utilisée ci-dessous ; les rares renvois à des fichiers de preuve cités dans les documents de spécification sont des citations, pas des lectures.
- **Les chiffres de performance, de version (MSRV, crates) et de licence sont des décisions de conception, pas des mesures.** Ils sont à valider dans le jalon 0 de la feuille de route.

### 0.1 Documents d'entrée lus

| Document | Lu | Appartient à la liste autorisée |
|---|---|---|
| `prompts/README.md` | oui | oui |
| `rea_output/STATE.md` | oui | oui |
| `rea_output/00_etat_de_l_art.md` (sections 5.1 à 5.4 lues en détail, titres des autres) | oui | oui |
| `rea_output/10_format_sas7bdat.md` (sections 1, 3, 5, 6, 7, 8, 9 en détail, titres du reste) | oui | oui |
| `rea_output/15_compression.md` (sections 2.1 et 5 en détail, titres du reste) | oui | oui |
| `rea_output/20_ecriture.md` (entièrement, rédigé dans la session précédente) | oui | oui |
| `rea_output/30_formats_encodages.md` (sections 0, 1 en tête, 3, 6.2 en détail, titres du reste) | oui | oui |
| `rea_output/60_inconnues.md` (U01 à U25 lues, titres de C, F et W) | oui | oui |
| `corpus/MANIFEST.md` (en-tête et notes) | oui | oui |

Non lus : `src/`, `rea_output/evidence/`, toute sortie de décompilation, tout code source de ReadStat, Parso, pandas ou autre implémentation (la section 5.1 de `00` autorise la lecture libre de plusieurs d'entre elles ; **la conception n'en a lu aucune**).

## 1. Vue d'ensemble

Le driver lit et écrit les fichiers `.sas7bdat` et expose leurs données à cinq usages. Le **cœur** décode et encode le format à partir de tampons d'octets, sans I/O, sans async, sans Arrow ; tout le reste (accès aux fichiers, Arrow, SQL, langages) est dans des crates adaptatrices qui dépendent du cœur.

```
                    ┌────────────────────────────────────────────────────────┐
                    │  consommateurs : DataFusion · pyarrow/pandas/polars ·   │
                    │  R (haven) · Arrow Flight / Dremio · CLI                │
                    └──────────────▲─────────────────────────────────────────┘
                                   │ RecordBatch (Arrow), data.frame, flux Flight
   uniformat-sas-datafusion  uniformat-sas-py  uniformat-sas-r  uniformat-sas-flight  uniformat-cli
                    └──────┬──────┴──────┬─────────────┘
                           │             │
                  uniformat-sas-arrow     (R sans Arrow : lit via uniformat-sas-io)
                           │
                    uniformat-sas-io        ByteSource (tranches, plages), mmap, object_store
                           │
                    sas7bdat           cœur : en-tête, pages, sous-en-têtes, lignes,
                                       décompression, formats → types, encodages,
                                       écriture (encodage de pages), erreurs
```

## 2. Découpage en crates et graphe de dépendances

| Crate | Rôle | Dépend de | Dépendances externes autorisées |
|---|---|---|---|
| `sas7bdat` (cœur) | modèle (`FileHeader`, `TableMeta`, `Column`), analyseurs sans I/O, décodage des lignes, RLE/RDC, classification des formats, table des encodages, planificateur d'écriture (construction des pages en mémoire), erreurs | — | `thiserror`, `encoding_rs`, `memchr` ; `proptest` en dev seulement |
| `uniformat-sas-io` | trait `ByteSource` (lecture de plages), implémentations tranche, fichier (`pread`), `mmap` (feature), pilotage de la lecture des métadonnées puis des pages ; écriture vers `Write + Seek` | `sas7bdat` | `memmap2` (feature `mmap`), `bytes` |
| `uniformat-sas-arrow` | schéma Arrow depuis `TableMeta`, flux de `RecordBatch`, missing spéciaux, écriture depuis Arrow ; adaptateur async `object_store` (feature) | `sas7bdat`, `uniformat-sas-io` | `arrow-array`, `arrow-schema`, `arrow-buffer`, `arrow-data` ; `object_store`, `futures` (feature `object-store`) |
| `uniformat-sas-datafusion` | `TableProvider` / `ExecutionPlan`, projection, `LIMIT`, partitions par plages de pages, statistiques | `uniformat-sas-arrow` | `datafusion` |
| `uniformat-sas-py` | module Python (PyO3, maturin) : lecture, écriture, métadonnées | `uniformat-sas-arrow` | `pyo3`, `arrow` (C Data Interface) |
| `uniformat-sas-r` | paquet R (extendr) : `data.frame` aux attributs `haven` | `uniformat-sas-io` (pas Arrow) | `extendr-api` |
| `uniformat-sas-flight` | (jalon 7) serveur Arrow Flight : schéma seul, `FlightInfo` par plages de pages, `do_get` | `uniformat-sas-arrow` | `arrow-flight`, `tonic`, `tokio` |
| `uniformat-cli` | `sas7bdat info / schema / head / convert` | `uniformat-sas-arrow` | `clap` |

**Dépendances autorisées dans le cœur** (ADR 0001) : `thiserror`, `encoding_rs`, `memchr`. Interdits dans le cœur : `std::fs`, `std::net`, `tokio`/`futures`, `memmap2`, `arrow*`, `datafusion`, `pyo3`, `extendr`, `rayon`, toute crate qui exige `unsafe` dans notre code. Le cœur compile avec `#![forbid(unsafe_code)]`.

Graphe (une flèche = « dépend de ») : `-cli` → `-arrow`; `-datafusion` → `-arrow`; `-py` → `-arrow`; `-flight` → `-arrow`; `-arrow` → `-io`; `-r` → `-io`; `-io` → `sas7bdat`. **Aucune flèche ne part du cœur** ni de `uniformat-sas-io` vers une crate située au-dessus d'elle.

## 3. Flux de données

```
fichier ──ByteSource──► en-tête (HL octets) ──► FileHeader (w, a1, endian, encodage, HL, P, page_count)
                                               │
                         pages de métadonnées  ▼  (première passe : page 0, 1, … jusqu'à ce que colonnes et RowSize soient complets)
                         MetaScanner ──► TableMeta (colonnes, RowSize, compression, textes décodés)  ──► schéma Arrow (aucune page de données lue)
                                               │
                         PageIndex (plan des pages : type, lignes physiques, plage de pages) ◄── en-têtes de pages (40 octets chacun)
                                               │
        plage de pages [p0, p1) ──► PageDecoder (décompression par ligne, bitmap de suppression) ──► RowRef (octets de ligne)
                                               │
                         ColumnDecoder (projection : seules les colonnes demandées) ──► builders Arrow ──► RecordBatch
                                               │
                         consommateurs : DataFusion (partitions), Python/R (flux), Flight (plages), CLI
```

Points structurants tirés de la spécification :

1. **Le schéma ne dépend que de l'en-tête et des sous-en-têtes** (`10` §4 : ColumnSize, ColumnText, ColumnName, ColumnAttributes, FormatAndLabel). Pour un fichier compressé, la détection se fait par le littéral `SASYZCRL`/`SASYZCR2` d'un bloc de texte (`15` §1). La première passe lit les pages à partir de la page 0 jusqu'à avoir `column_count` colonnes ; les métadonnées d'une table de 5 000 colonnes tiennent sur 5 pages, celles d'une table ordinaire sur la page 0 seule. Aucune page de lignes n'est nécessaire.
2. **Unité de lecture = page** de `P` octets, à `HL + k × P` (`10` §1, §3). Une plage de pages est donc une plage d'octets calculable sans lire le fichier : c'est ce qui permet `mmap`, les lectures par plages `object_store`, la lecture parallèle et le partage Flight.
3. **Lignes non compressées** : contiguës dans les pages de données `0x0100` et mixtes `0x0200`/`0x0280` (`10` §3.3). **Lignes compressées** : un sous-en-tête par ligne dans une page `0x0000`, décompressée seule (`15` §2.1) ; les pages `0x9000` sont ignorées. Les deux chemins produisent la même abstraction `RowRef`.
4. **Lignes supprimées** : bitmap par page (`10` §3.5), jamais exposées (ADR 0004).
5. **Numériques** : double, ou `n` octets de poids fort avec `n` de 3 à 7 (`10` §5.2), manquants selon la famille (`10` §5.3). **Caractères** : octets de l'encodage de la table, blancs finaux retirés selon l'option (`10` §5.4, `30` §6).

## 4. API publique esquissée (signatures seulement)

### 4.1 Cœur `sas7bdat` (sans I/O)

```rust
// --- modèle ---
pub struct FileHeader { /* w, a1, endianness, encoding_code, header_length, page_length, page_count,
                          created, modified, sas_release, host, table_name, file_type */ }
pub struct TableMeta {
    pub header: FileHeader,
    pub table_name: String, pub table_label: Option<String>,
    pub row_count: u64, pub deleted_count: u64, pub row_length: u32,
    pub compression: Compression,                 // None | Char | Binary
    pub encoding: Encoding,                       // code d'en-tête + nom + encodage de décodage
    pub columns: Vec<Column>,
}
pub struct Column {
    pub index: usize, pub name: String, pub label: Option<String>,
    pub kind: ColumnKind,                         // Numeric { length: u8 } | Char { length: u32 }
    pub offset: u32,
    pub format: Option<SasFormat>,                // name, width, decimals
    pub informat: Option<SasFormat>,
    pub logical: LogicalType,                     // Number | Date | DateTime | Time | Text (table de 30 §1)
    pub indexed: bool,
}
pub enum NumValue { Value(f64), Missing(MissingKind) }          // MissingKind: Dot | Underscore | Letter(u8)
pub struct Limits { pub max_page_length: u32, pub max_columns: u32, pub max_text_bytes: usize, pub max_row_length: u32 }

// --- analyse sans I/O ---
pub struct HeaderParser;
impl HeaderParser { pub fn parse(bytes: &[u8], limits: &Limits) -> Result<FileHeader>; }           // HL octets
pub struct MetaScanner;
impl MetaScanner {
    pub fn new(header: &FileHeader, limits: &Limits) -> Self;
    pub fn feed_page(&mut self, page_index: u64, page: &[u8]) -> Result<ScanProgress>;             // Done | NeedMore
    pub fn finish(self) -> Result<TableMeta>;                                                      // schéma complet
}
pub struct PageInfo { pub index: u64, pub kind: PageKind, pub physical_rows: u32, pub has_deleted: bool }
pub fn page_info(meta: &TableMeta, page_index: u64, page_header: &[u8]) -> Result<PageInfo>;      // 40 octets suffisent
pub struct PageDecoder<'m> { /* meta, scratch pour la décompression */ }
impl<'m> PageDecoder<'m> {
    pub fn new(meta: &'m TableMeta) -> Self;
    pub fn rows<'p>(&'p mut self, page_index: u64, page: &'p [u8]) -> Result<PageRows<'p>>;       // lignes visibles uniquement
}
impl<'p> RowRef<'p> {
    pub fn num(&self, col: &Column) -> NumValue;
    pub fn bytes(&self, col: &Column) -> &'p [u8];                                                 // octets bruts, blancs finaux compris
    pub fn text(&self, col: &Column, policy: &TextPolicy) -> Result<Cow<'p, str>>;
}
// --- décompression ---
pub fn rle_decompress(src: &[u8], out: &mut [u8]) -> Result<()>;
pub fn rdc_decompress(src: &[u8], out: &mut [u8]) -> Result<()>;
pub fn rle_compress(row: &[u8], out: &mut Vec<u8>);
pub fn rdc_compress(row: &[u8], out: &mut Vec<u8>);
// --- classification et dates ---
pub fn classify_format(name: &str, kind_hint: StorageKind) -> LogicalType;                         // table de 30 §1
pub fn sas_date_to_unix_days(v: f64) -> Option<i32>;  pub fn sas_datetime_to_unix_micros(v: f64) -> Option<i64>;
// --- écriture (construction en mémoire, aucun I/O) ---
pub struct WriteSchema { /* colonnes, encodage, taille de page, compression, options de création */ }
impl WriteSchema { pub fn validate(&self) -> Result<ValidatedSchema, SchemaError>; }               // limites de 20 §8, avant tout octet
pub struct FileBuilder;
impl FileBuilder {
    pub fn new(schema: ValidatedSchema, opts: WriteOptions) -> Self;
    pub fn push_row(&mut self, row: &[CellValue]) -> Result<(), WriteError>;                       // CellValue: Num(NumValue) | Text(&str|&[u8])
    pub fn take_ready_pages(&mut self) -> Vec<PageBytes>;                                          // pages complètes, dans l'ordre
    pub fn finish(self) -> Result<FinalPages, WriteError>;                                         // page 0 (RowSize, compteurs), en-tête, marqueur 0x9000
}
```

### 4.2 `uniformat-sas-io` (I/O synchrone et plages)

```rust
pub trait ByteSource { fn len(&self) -> u64;  fn read_range(&self, offset: u64, len: usize) -> io::Result<Bytes>; }
impl ByteSource for &[u8] / File / MmapSource / (feature object-store) ObjectStoreSource;
pub struct SasFile<S: ByteSource> { /* meta, header, source */ }
impl<S: ByteSource> SasFile<S> {
    pub fn open(source: S, opts: OpenOptions) -> Result<Self>;                 // lit HL + pages de métadonnées seulement
    pub fn meta(&self) -> &TableMeta;                                          // métadonnées complètes, sans lecture des données
    pub fn page_plan(&self, mode: PlanMode) -> Result<PagePlan>;               // Fast : aucune lecture, vérifié page par page ; Full : en-têtes de pages (40 octets)
    pub fn read_pages(&self, pages: &Range<u64>) -> Result<Cow<[u8]>>;          // une seule plage d'octets (réalisé en J4)
}
pub struct SasWriter<W: Write + Seek> { /* FileBuilder + sink */ }
impl<W: Write + Seek> SasWriter<W> {
    pub fn new(sink: W, schema: ValidatedSchema, opts: WriteOptions) -> Result<Self>;
    pub fn write_row(&mut self, row: &[CellValue]) -> Result<()>;
    pub fn finish(self) -> Result<W>;                                          // réécrit en-tête et page 0 (compteurs finaux)
}
```

### 4.3 `uniformat-sas-arrow`

```rust
pub struct ReadOptions { pub trailing_blanks: TrailingBlanks, pub missing: MissingMode, pub dates: DateMode,
                         pub invalid_utf8: InvalidUtf8, pub encoding_override: Option<EncodingName>,
                         pub batch_rows: usize, pub strictness: Strictness, pub decimal_for_fixed: bool }
pub fn arrow_schema(meta: &TableMeta, opts: &ReadOptions) -> SchemaRef;      // USAGE 2 : schéma sans lire les données
pub struct BatchReader<S: ByteSource> { /* RecordBatchReader */ }
impl<S: ByteSource> BatchReader<S> {
    pub fn new(file: Arc<SasFile<S>>, pages: Range<u64>, projection: Option<Vec<usize>>, limit: Option<u64>, opts: ReadOptions) -> Result<Self>;
    pub fn with_group(file: Arc<SasFile<S>>, group: &PageGroup, plan: Arc<PagePlan>, projection: Option<Vec<usize>>, limit: Option<u64>, opts: ReadOptions) -> Result<Self>;   // partition vérifiée contre le plan
}
impl Iterator for BatchReader<_> { type Item = Result<RecordBatch>; }       // streaming par groupe de pages
// feature object-store (réalisé en J4) : ObjectFile::open(store, path, opts).await ; .stream(&PageGroup, plan, projection, limit) -> BoxStream<Result<RecordBatch>> ; .stream_all(..)
pub struct ObjectFile;
pub struct ArrowWriter<W: Write + Seek> { /* SasWriter + convertisseur */ }
impl<W: Write + Seek> ArrowWriter<W> {
    pub fn try_new(sink: W, schema: SchemaRef, opts: ArrowWriteOptions) -> Result<Self, SchemaError>;   // validation avant le 1er octet
    pub fn write(&mut self, batch: &RecordBatch) -> Result<()>;
    pub fn close(self) -> Result<W>;
}
```

### 4.4 `uniformat-sas-datafusion`

```rust
pub struct SasTableProvider { /* Arc<SasFile<..>>, ReadOptions, target_partitions */ }
impl TableProvider for SasTableProvider {
    fn schema(&self) -> SchemaRef;                                             // sans lecture des données
    async fn scan(&self, state: &dyn Session, projection: Option<&Vec<usize>>, filters: &[Expr], limit: Option<usize>) -> Result<Arc<dyn ExecutionPlan>>;
    fn supports_filters_pushdown(&self, f: &[&Expr]) -> Result<Vec<TableProviderFilterPushDown>>;   // Unsupported : pas de statistiques de colonnes dans le format
}
pub struct SasExec { /* partitions = plages de pages disjointes */ }
impl ExecutionPlan for SasExec { fn statistics(&self) -> Result<Statistics>; /* num_rows exact = row_count - deleted_count */ }
```

### 4.5 Python et R

```python
# uniformat_io.sas (PyO3 + maturin, abi3-py39) — signatures réalisées en J5
def read_sas(path, *, columns=None, limit=None, trailing_blanks="trim", missing="null", missing_suffix="__missing",
             dates="native", timestamp_unit="us", time="time", out_of_range="null", invalid_bytes="replace",
             empty_as_null=False, encoding=None, strict=False, batch_rows=8192, plan="fast") -> "pyarrow.RecordBatchReader"  # flux PyCapsule
def read_sas_table(...) -> "pyarrow.Table"          # mêmes options ; émet les avertissements de lecture
def read_sas_pandas(path, *, keep_missing_columns=False, ...) -> "pandas.DataFrame"   # attrs["sas"], attrs["sas_missing"]
def read_sas_polars(path, *, missing_as="uint8", ...) -> "polars.DataFrame"
def schema(path, **kw) -> "pyarrow.Schema"          # sans lecture de page de données
def metadata(path, **kw) -> dict                    # {"table": {...}, "columns": [...]}
def write_sas(path, data, *, compression=None, encoding="utf-8", column_labels=None, formats=None, informats=None, lengths=None,
              page_length=65536, grow_page=True, on_unrepresentable="error", missing_suffix="__missing", table_name=None,
              file_code=None, created=None) -> dict    # J6 : data = Table/RecordBatchReader/pandas/polars ; rapport
```

```r
# paquet R uniformatsas (extendr) — signatures réalisées en J5
read_sas7bdat(path, col_select = NULL, n_max = Inf, encoding = NULL, trim_ws = TRUE, strict = FALSE)  # data.frame : attributs label, format.sas, informat.sas ; NA étiquetés (haven::na_tag)
sas7bdat_meta(path, encoding = NULL, strict = FALSE)           # liste : table, columns
write_sas7bdat(data, path, compression = c("none", "char", "binary"), encoding = "UTF-8", page_length = 65536L,
               on_unrepresentable = c("error", "replace", "truncate"))      # J6 : NA étiquetés haven -> manquants spéciaux
```

Ligne de commande (`uniformat-cli`) : `sas7bdat info|schema|count|csv FICHIER`. Chemins locaux seulement pour Python et R ; les URL restent réservées aux adaptateurs Rust (`ObjectFile`, DataFusion).

### 4.6 Couverture des cinq usages

| Usage | Crate(s) | Esquisse |
|---|---|---|
| 1. Bibliothèque Rust lecture/écriture, métadonnées complètes (labels, formats, informats, encodage, horodatages, compression, nom et label de table) | `sas7bdat`, `uniformat-sas-io` | `SasFile::meta() -> &TableMeta`, `SasWriter`, `FileHeader` (horodatages, release) |
| 2. Arrow en streaming par groupe de pages, schéma sans données | `uniformat-sas-arrow` | `arrow_schema(&TableMeta)`, `BatchReader` sur `Range<u64>` de pages |
| 3. Python (pyarrow/polars/pandas) et R (haven) | `uniformat-sas-py`, `uniformat-sas-r` | 4.5 ; métadonnées dans les champs Arrow (ADR 0003) et dans les attributs R |
| 4. SQL DataFusion | `uniformat-sas-datafusion` | `SasTableProvider` (projection, `LIMIT`, partitions par plages de pages, statistiques) |
| 5. Flight / Dremio | `uniformat-sas-flight` | `get_flight_info` (schéma seul, un endpoint par plage de pages), `do_get(ticket)` ; ticket JSON = fichier + plage de pages + options (projection, `LIMIT`…), revalidé à chaque appel ; binaire `uniformat-sas-flight-server <dossier>` (J7) |

## 5. Table de traçabilité : spécification → module

Le crate et le module sont ceux du cœur `sas7bdat` sauf indication. « n/a v1 » = hors du périmètre de la première version, avec la raison. L'inventaire de départ est celui des titres de sections de `10`, `15`, `20`, `30` (tâche 1.4).

### 5.1 `10_format_sas7bdat.md`

| Section | Objet | Module / crate |
|---|---|---|
| 0, 0.1, 0.2 | statut, paires différentielles, notation | `docs/` (hypothèses ; tests `tests/corpus`) |
| 1 | vue d'ensemble | `header`, `page` (modèle) |
| 2.1 | octets 0 à 83 (magic, drapeaux 64 bits/a1/endian, copies, encodage, nom, type) | `header::ident` |
| 2.2 | horodatages, tailles, versions, `file_code` | `header::times`, `header::sizes` |
| 3 / 3.1 | pages, en-tête de page | `page::header` |
| 3.2 | pointeurs de sous-en-têtes | `page::pointer` |
| 3.3 | position et nombre des lignes | `page::rows` |
| 3.4 | pages d'index (amd) | `page::kind` (reconnue, ignorée ; index hors périmètre) |
| 3.5 | lignes supprimées | `page::deleted` |
| 4 / 4.1 | sous-en-têtes, signatures | `subheader::signature` |
| 4.2 | RowSize | `subheader::row_size` |
| 4.3 | ColumnSize | `subheader::column_size` |
| 4.4 | SubheaderCounts | `subheader::counts` (lu, ignoré ; écrit : `writer::fields`) |
| 4.5 | ColumnText | `subheader::column_text` |
| 4.6 | ColumnName | `subheader::column_name` |
| 4.7 | ColumnAttributes | `subheader::column_attrs` |
| 4.8 | FormatAndLabel | `subheader::format_label` |
| 4.9 | ColumnList | `subheader::column_list` (ignoré) |
| 5 / 5.1 | lignes et valeurs, disposition | `row` |
| 5.2 | numériques (3 à 8 octets) | `row::numeric` |
| 5.3 | manquants | `row::missing` |
| 5.4 | caractères | `row::text`, `encoding` |
| 5.5 | lignes supprimées | `page::deleted` |
| 6 | procédure de lecture | `uniformat-sas-io::SasFile` (pilotage) + `MetaScanner`, `PageDecoder` |
| 7 | contrôles à l'ouverture | `validate` (contrôles du driver, pas ceux de SAS) ; ADR 0006 (U25) |
| 8 | cibles de décompilation | aucun module : renvoi à `60` ; ADR 0006 |
| 9 | couverture et complétude | tests : `tests/header_coverage`, `tests/corpus` |
| 10 | renvois | n/a |

### 5.2 `15_compression.md`

| Section | Module / crate |
|---|---|
| 0 | statut — ADR 0006 |
| 1 identification des algorithmes (littéraux) | `compress::detect` |
| 2 / 2.1 détection et stockage (pointeur `comp = 4`, ligne brute, `0x9000`) | `compress::rowptr`, `page::kind` |
| 3 RLE | `compress::rle` |
| 4 RDC | `compress::rdc` |
| 5 règles du compresseur, `obligatoire` / `libre` | `writer::compress_policy` (R1–R12) |
| 6 vecteurs de test | `tests/vectors` (338 + 367 vecteurs) |
| 7 décompresseur et compresseur de référence | oracle des tests (`tests/oracle`), non intégré au driver |
| 8 cibles de décompilation | aucun module : ADR 0006 |

### 5.3 `20_ecriture.md`

| Section | Module / crate |
|---|---|
| 0 statut et portée | ADR 0005 |
| 1 lecteurs et résultats | `tests/roundtrip` (suites SAS optionnelle, pandas, ReadStat) |
| 2 notation | — |
| 3 entrées de l'écrivain | `writer::schema` |
| 4.1 table non compressée (disposition, textes, sous-en-têtes, pages, RowSize, en-tête) | `writer::layout`, `writer::text`, `writer::subheaders`, `writer::pages`, `writer::header` |
| 4.2 table compressée (littéral, pointeurs, marqueur `0x9000`, RowSize) | `writer::compressed` |
| 4.3 ordre de contrôle avant écriture | `writer::validate` |
| 5 table des champs, 5.1 en-tête de fichier | `writer::fields::header` (tables `const`) |
| 5.2 en-tête de page | `writer::fields::page` |
| 5.3 pointeurs de sous-en-têtes | `writer::fields::pointer` |
| 5.4 `RowSize` | `writer::fields::row_size` |
| 5.5 autres sous-en-têtes | `writer::fields::subheaders` |
| 5.6 page marqueur `0x9000` | `writer::compressed::marker` |
| 6.1 ordre des sous-en-têtes, taille de page, en-tête | `writer::layout` |
| 6.2 répartition des lignes, compteurs | `writer::pages` |
| 6.3 numériques et manquants | `writer::cells` |
| 6.4 textes et encodages | `writer::text`, `encoding` |
| 7 cas non supportés | `Error::Unsupported` ; ADR 0005 |
| 8 limites | `writer::limits` |
| 9 inconnues | ADR 0006 |
| 10 reproduction et traçabilité | `tests/roundtrip`, `CONTRIBUTING.md` |

### 5.4 `30_formats_encodages.md`

| Section | Module / crate |
|---|---|
| 0 statut | ADR 0006 |
| 1 table de classification format → Arrow | `formats::classify` (table générée à la compilation depuis un fichier de données) ; mapping Arrow : `uniformat-sas-arrow::types` (ADR 0003) |
| 2 inventaire des formats et informats natifs / 2.1 NLS | `formats::classify` (liste de noms) ; NLS : repli sur le type de stockage |
| 3 informats | `subheader::format_label` (recopiés, jamais interprétés) |
| 4 algorithmes d'affichage, 4.1 conventions communes | **n/a v1** (affichage non requis par les cinq usages) ; jalon optionnel `sas7bdat-fmt` |
| 4.2 `w.d`, `BEST`, `E`, `Z` | n/a v1 (`sas7bdat-fmt`) |
| 4.3 `COMMA`, `DOLLAR`, `PERCENT`, `NEGPAREN` | n/a v1 (`sas7bdat-fmt`) |
| 4.4 dates, datetimes, heures | conversion d'époque : `formats::epoch` (cœur) ; affichage : n/a v1 |
| 4.5 `$CHARw.`, `$w.`, `$QUOTEw.` | n/a v1 (`sas7bdat-fmt`) |
| 4.6 ce qui n'est pas reproduit (F02–F06) | n/a v1 ; ADR 0006 |
| 5 catalogues `.sas7bcat`, 5.1 en-tête | **n/a v1** (formats utilisateur traités comme inconnus, section 1 de `30`) ; futur `catalog::header` |
| 5.2 répertoire | n/a v1 ; futur `catalog::directory` |
| 5.3 entrées | n/a v1 (la spec ne décode ni plages ni options : F07 à F09) |
| 5.4 ce qui manque | ADR 0006 (F07–F09) |
| 6 encodages, 6.1 table des encodages | `encoding::table` |
| 6.2 octets invalides, troncature multi-octets | `encoding::policy`, `writer::text` |
| 7 cibles de décompilation | aucun module : ADR 0006 |
| 8 renvois | n/a |

## 6. Stratégie de tests

### 6.1 Tests d'intégration sur le corpus (tâche 4.1)

- `corpus/MANIFEST.md` liste chaque fichier avec sa famille, le paramètre qui varie et l'empreinte SHA-256 ; les CSV de référence sont à côté des `.sas7bdat` (`files/`, `external/`). Le test d'intégration lit chaque `.sas7bdat` non compressé, compressé ou externe et compare **par nom de colonne** (l'ordre du CSV n'est pas celui du fichier, `10` §0) aux CSV avec une tolérance relative de `1e-13` sur les flottants (14 chiffres significatifs dans les CSV, `10` §5.2).
- Métadonnées : comparées aux sorties `PROC CONTENTS` conservées avec le corpus (nom, type, longueur, format, informat, label, observations, encodage).
- Suites de familles : `base`, `types`, `valeurs`, `metadonnees`, `encodage`, `taille` (pages 4 096 à 131 072), `suppression`, `compression` (30 tables réellement compressées sur 126 : les 96 autres se lisent comme des tables ordinaires), `plateforme` (32 bits, big endian), `index`, `externe` (21 fichiers publics compressés et non compressés).
- Les fichiers du corpus produits par le SAS complet **ne sont pas redistribués** dans le dépôt public (ADR 0007) : la suite d'intégration est activée par `UNIFORMAT_CORPUS=<dossier>` ; le dépôt public embarque seulement les fichiers sous licence redistribuable de `00` §5.3 (datareader, Parso, pandas, pyreadstat, ReadStat).

### 6.2 Round-trip (tâche 4.2)

`ArrowWriter` / `SasWriter` écrit un fichier ; il est relu par (1) le driver, (2) ReadStat (`pyreadstat`), (3) pandas, (4) Parso (JVM) et (5) **SAS** (`PROC CONTENTS`, `PROC PRINT`, `data _null_; set`) dans l'environnement Linux x86-64 défini par `setup-sas-oracle-corpus` (`oracle/run_sas.sh`), **suite optionnelle** (`SAS7BDAT_SAS_ORACLE=1`) car elle dépend de Docker amd64 (émulation lente sur Mac). Cas : ceux de la matrice de `20` §1 (49 cas × 5 lecteurs), y compris latin1/wlatin1, pages de 4 096 à 1 Mio, tables compressées de 0 à 255 lignes. Résultats attendus : ceux de `write_matrix.md` ; les 9 « KO connus » de la matrice deviennent des assertions négatives documentées (limite de 255 lignes compressées, pyreadstat et code 29, noms de 33 octets…). Tout écart ouvre une correction de la spécification avant celle du code.

### 6.3 Tests par propriétés (tâche 4.3)

`proptest` : (a) décodage de valeurs : pour tout `f64` et toute largeur 3 à 8, `decode(encode(x))` = `x` tronqué à la précision stockée ; manquants `.`, `._`, `.A`–`.Z` aller-retour exact ; (b) compression : pour toute ligne aléatoire (et familles structurées : suites constantes de 2 à 4 120 octets, motifs de période 3, 4, 7, mélanges), `decompress(compress(x)) == x` pour RLE et RDC ; (c) pages : une table aléatoire écrite puis relue redonne ses lignes pour des tailles de page de 4 096 à 131 072 ; (d) dates : `sas_date_to_unix_days` est monotone sur la plage valide 1582-01-01…20000 et refuse hors plage.

### 6.4 Fuzzing (tâche 4.4)

`cargo-fuzz`, cibles : `fuzz_header` (`HeaderParser::parse`), `fuzz_pages` (`MetaScanner` puis `PageDecoder::rows` sur des pages arbitraires avec un `TableMeta` valide et avec un `TableMeta` issu du fuzzeur), `fuzz_rle`, `fuzz_rdc` (décompresseurs avec `out.len()` arbitraire jusqu'à une borne), `fuzz_writer_roundtrip`. **Propriétés** : (1) aucune panique, aucun dépassement d'indice, aucune arithmétique qui déborde (build avec `overflow-checks`) ; (2) **allocations bornées** : toute allocation dérivée d'un champ du fichier (`page_length`, `column_count`, `row_length`, longueurs de textes, `page_count`) est plafonnée par `Limits` (par défaut : page ≤ 16 Mio, colonnes ≤ 100 000, ligne ≤ 64 Mio, texte ≤ taille du fichier) et par la taille réelle de la source ; (3) la décompression n'écrit jamais au-delà de `row_length`. Corpus de graines : les fichiers du corpus tronqués, avec champs d'en-tête mutés (les 96 champs mutés de `20` donnent des graines pertinentes). Exécution en CI courte (60 s par cible) et nocturne longue.

### 6.5 Benchmarks (tâche 4.5)

`criterion` sur le même corpus (`taille_n50000`, `base_n3000`, `taille_wide300/5000`, fichiers compressés) : débit en Mio/s et lignes/s (a) driver → `RecordBatch`, (b) `pyreadstat.read_sas7bdat`, (c) `pandas.read_sas`, (d) ReadStat C via `pyreadstat` en mode `output_format=dict`. Mesures : lecture complète, projection de 2 colonnes, `LIMIT 1000`, parallèle à 1/4/8 partitions, compressé RLE et RDC. Critère : voir `docs/roadmap.md`. Aucun chiffre n'est annoncé avant mesure.

## 7. Contraintes transversales

- **Allocations bornées et pas de `unsafe`** : ADR 0007.
- **Déterminisme** : à entrée égale, l'écriture produit des pages identiques, sauf `file_code` et horodatages (paramétrables pour les tests).
- **Journalisation** : le cœur ne journalise pas ; il remonte une liste typée d'avertissements (`ReadReport`) que l'adaptateur transforme en `log`/`warnings.warn`/`warning()`.
- **Documentation des inconnus** : chaque avertissement porte l'identifiant de `60_inconnues.md` (par exemple `U19`) pour retrouver la décision.

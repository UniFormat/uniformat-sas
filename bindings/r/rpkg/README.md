# uniformatsas (paquet R)

Lit un fichier SAS `.sas7bdat` avec le driver Rust du dépôt.

```r
library(uniformatsas)
df <- read_sas7bdat("t.sas7bdat", col_select = c("x", "d"), n_max = 1000)
attr(df$x, "label"); attr(df$x, "format.sas")      # attributs compatibles haven
haven::na_tag(df$x)                                # manquants spéciaux : "_", "a" à "z"
sas7bdat_meta("t.sas7bdat")                        # table et colonnes, sans lire de ligne
write_sas7bdat(df, "out.sas7bdat", compression = "char")   # attributs label/format.sas, NA étiquetés, Date/POSIXct/hms
```

Dates : `Date` ; date-heures : `POSIXct` (UTC) ; heures : `hms::hms`. Erreurs : conditions de classe `sas7bdat_error`.

Installation depuis le dépôt (exige R, `cargo` ≥ 1.97) : `R CMD INSTALL crates/uniformat-sas-r/rpkg`. Si le volume du projet ne
supporte pas le verrouillage de fichiers, définir `CARGO_TARGET_DIR` vers un dossier local. Tests : `UNIFORMAT_CORPUS=<corpus>
Rscript -e 'testthat::test_dir("crates/uniformat-sas-r/rpkg/tests/testthat", package = "uniformatsas")'` (exige `haven`, `hms`).

Paquet **non publié** (décision NULL en attente, ADR 0007) .

## XPORT v5

```r
study <- read_xpt("study.xpt", encoding = "UTF-8", member = 1)
write_xpt(study, "copy.xpt")
```

`member` accepte un nom ou un index à partir de 1. `n_max=0` pour le premier
membre ne lit que les métadonnées. `max_bytes` borne les colonnes matérialisées
(256 Mio par défaut). L'encodage est strict ; `latin1` et `windows-1252` sont
explicites. Les NA étiquetés de haven conservent `._` et `.A` à `.Z`.
L'écriture est atomique et les dates d'en-tête déterministes ; noms, labels et
valeurs dépassant les limites v5 provoquent une erreur. Les tests `test-xpt.R`
comparent aussi les fichiers à `haven::read_xpt` et `haven::write_xpt(version=5)`.

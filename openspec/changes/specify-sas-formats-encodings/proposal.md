## Why

Dans un `.sas7bdat`, un format n'est qu'un **nom** (+ largeur, décimales) attaché à une colonne : sans table de correspondance, le driver Rust ne peut ni déduire le type logique (date, datetime, heure, décimal…), ni afficher une valeur comme SAS, ni lire les formats utilisateur stockés dans les catalogues `.sas7bdat`, ni décoder le texte selon l'encodage du fichier. Ce changement (prompt 04) produit cette spécification, condition préalable à la conception du driver (`design-rust-driver`).

## What Changes

- Inventaire des formats et informats natifs connus de `src/sasexe/tkefmt.so` et des modules `tkefmf*.so` / `tkefmi*.so`, avec le module de chaque famille.
- **Table de classification** format → catégorie logique → type Arrow cible (date en jours depuis 1960-01-01, datetime en secondes depuis 1960-01-01T00:00:00, heure en secondes, numérique, décimal à échelle fixe, caractère), formats ambigus signalés ; placée en tête du livrable car directement consommée par le driver.
- Algorithmes d'affichage des formats courants (`w.d`, `BEST`, `E`, `COMMA`, `DOLLAR`, `PERCENT`, `Z`, `DATE`, `DATETIME`, `TIME`, `YYMMDD`, `DDMMYY`, `MMDDYY`, ISO 8601 `E8601DA`/`E8601DT`…, `$CHAR`, `$`) : arrondi, largeur insuffisante (`*`, bascule en notation E), valeurs manquantes ; vérifiés contre `PROC PRINT` du corpus.
- Formats NLS (`NLDATE`…) et localisés : listés uniquement, non spécifiés (hors périmètre).
- Spécification de la structure des catalogues `.sas7bcat` de formats utilisateur (en-tête, répertoire des entrées, entrées `FORMAT`/`FORMATC`/`INFMT`/`INFMTC`, plages, libellés), par comparaison de fichiers d'abord puis décompilation ciblée ; lecteur de référence validé contre `CNTLOUT=`, ou partie explicitement marquée bloquée avec sa raison.
- Table complète code d'encodage de l'en-tête → nom d'encodage SAS → encodage standard WHATWG/IANA (utilisable avec `encoding_rs`), et comportement de SAS face aux octets invalides et à la troncature d'un caractère multi-octets en fin de champ.
- Livrables : `rea_output/30_formats_encodages.md`, vérificateurs dans `rea_output/tools/` (affichage, catalogue, encodage), mises à jour de `rea_output/60_inconnues.md`, `rea_output/evidence/` et `rea_output/STATE.md`.
- Hors périmètre : écriture des catalogues, spécification des formats NLS/localisés, tout code du driver Rust, toute modification de `src/`.

## Capabilities

### New Capabilities
- `sas-formats-catalog` : inventaire et classification des formats/informats natifs SAS vers les types Arrow, algorithmes d'affichage des formats courants, et structure des catalogues `.sas7bcat` de formats utilisateur.
- `sas-encodings` : correspondance code d'encodage de l'en-tête → encodage SAS → encodage WHATWG/IANA, et comportement de SAS sur octets invalides et troncature multi-octets.

### Modified Capabilities
<!-- Aucune : aucune spec existante sous openspec/specs/. -->

## Impact

- **Dépendances amont** : `specify-sas7bdat-read-format` (02, `rea_output/10_format_sas7bdat.md` : emplacement des noms de format et du code d'encodage dans l'en-tête), `setup-sas-oracle-corpus` (01, oracle SAS et `corpus/MANIFEST.md`, disponibilité de `PROC FORMAT`), `survey-sas7bdat-prior-art` (00, ce qui est déjà publié, pour cibler la décompilation sur les seuls manques).
- **Parallélisme** : indépendant de `specify-sas7bdat-compression` (03), peut tourner en parallèle.
- **Consommateurs aval** : `specify-sas7bdat-writing` (05, conseillé) et `design-rust-driver` (06), qui ne lira que `rea_output/30_formats_encodages.md` et `60_inconnues.md`, jamais `evidence/`.
- **Systèmes** : oracle SAS (`src/`, lecture seule) exécuté en Linux x86-64 hors de l'hôte macOS ; outils MCP `rea` (skill `/reverse-engineer-anything`) ; corpus `corpus/` (catalogues générés ou `corpus/external/`).
- **Licence** : analyse des binaires SAS sous licence Apache, limitée à l'interopérabilité ; confirmation interne à consigner dans `rea_output/STATE.md` avant toute décompilation.

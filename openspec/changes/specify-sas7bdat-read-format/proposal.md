## Why

Le driver Rust visé (lecture/écriture `.sas7bdat`) ne peut être conçu qu'à partir d'une spécification du format, et non du code SAS. Or les descriptions publiques (ReadStat, Parso, pandas, PDF de Shotwell) ne sont que des hypothèses, incomplètes sur plusieurs champs. Une fois l'état de l'art établi (`survey-sas7bdat-prior-art`) et l'oracle SAS et son corpus disponibles (`setup-sas-oracle-corpus`), il faut produire une spécification complète et vérifiée de la structure d'un `.sas7bdat` **non compressé**, assez précise pour écrire un lecteur.

## What Changes

- Création de la spécification du format de lecture `rea_output/10_format_sas7bdat.md`, une entrée par champ selon le modèle de `prompts/README.md` (emplacement 32/64 bits, taille/type, sémantique, valeurs observées, écriture, preuve, confiance `confirmé` | `inféré` | `inconnu`), couvrant :
  - l'en-tête (magic, alignements 32/64 bits, endianness, plateforme, code d'encodage, horodatages epoch 1960, nom et type de table, versions SAS/OS, tailles d'en-tête et de page, nombre de pages, champs « réservés ») ;
  - les pages (meta, data, mix, amd, autres), l'en-tête de page, les pointeurs de sous-en-têtes, la position des lignes, le padding et l'alignement ;
  - les sous-en-têtes (signatures par variante 32/64 bits et LE/BE ; row size, column size, subheader counts, column text, column names, column attributes, format and label, column list, inconnus rencontrés) ;
  - les colonnes (type, longueur, offset, nom, label, format/informat via les références au texte) ;
  - les valeurs (numériques tronqués 3–8 octets, missing standard et spéciaux, caractères), les lignes supprimées ;
  - les contrôles effectués par SAS à l'ouverture (erreurs et messages de `src/sasmsg/`), en vue de l'écriture.
- Méthode imposée, dans l'ordre : état de l'art comme hypothèse `inféré` → analyse différentielle du corpus → décompilation ciblée (REA) des seuls champs restés `inféré`/`inconnu` et des règles de validation → vérification par un lecteur de référence Python écrit à partir de la seule spec.
- Outils réutilisables dans `rea_output/tools/` (scripts de comparaison, `ref_reader.py` sans ReadStat/pandas/Parso).
- Preuves dans `rea_output/evidence/` (bundle REA exporté, hexdumps annotés).
- Mise à jour de `rea_output/60_inconnues.md` et `rea_output/STATE.md`.
- Hors périmètre : compression (`specify-sas7bdat-compression`), formats et encodages au-delà du code d'encodage (`specify-sas-formats-encodings`), écriture (`specify-sas7bdat-writing`), conception Rust (`design-rust-driver`).

## Capabilities

### New Capabilities
- `sas7bdat-file-format` : spécification vérifiée de la structure d'un `.sas7bdat` non compressé (en-tête, pages, sous-en-têtes, colonnes, valeurs, lignes supprimées, contrôles à l'ouverture), méthode de production, lecteur de référence et critères de complétude.

### Modified Capabilities

## Impact

- Dépend de `survey-sas7bdat-prior-art` (`rea_output/00_etat_de_l_art.md`) et de `setup-sas-oracle-corpus` (environnement SAS Linux x86-64, `corpus/`, `corpus/MANIFEST.md`, CSV de référence).
- Débloque `specify-sas7bdat-compression` et `specify-sas-formats-encodings` (parallélisables), puis `specify-sas7bdat-writing` et `design-rust-driver` (qui ne lira que `10_format_sas7bdat.md`, pas `evidence/`).
- Fichiers créés ou modifiés : `rea_output/10_format_sas7bdat.md`, `rea_output/tools/`, `rea_output/evidence/`, `rea_output/60_inconnues.md`, `rea_output/STATE.md`.
- `src/` reste en lecture seule ; modules analysés : `sase7io`, `sase7opn`, `sase7l`, `sase7ld`, `sase7lu`, `sase7xgt`, `sase7xop`, `sase7xrt`, `src/dbcs/sasexe/sase7`, `sase7yse`, messages `src/sasmsg/`.
- Contrainte licence : la décompilation de binaires sous licence Apache n'intervient qu'après consignation dans `STATE.md` de la confirmation interne de couverture au titre de l'interopérabilité.

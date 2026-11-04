## Purpose

Spécifier comment le futur driver Rust identifie l'encodage d'un `.sas7bdat` à partir du code stocké dans l'en-tête, le traduit en encodage standard utilisable avec `encoding_rs`, et reproduit le comportement de SAS sur les octets invalides et les caractères multi-octets tronqués.

## ADDED Requirements

### Requirement: Table de correspondance des encodages
Le livrable `rea_output/30_formats_encodages.md` SHALL contenir une table complète code d'encodage dans l'en-tête → nom d'encodage SAS → encodage standard (nom WHATWG ou IANA utilisable avec `encoding_rs`), établie à partir de `tknlsenc.so`, `tknlsicv.so`, `src/nls/` et des fichiers du corpus, avec un niveau de confiance et une preuve par ligne.

#### Scenario: Couverture du corpus et des sources publiques
- **WHEN** on collecte tous les codes d'encodage présents dans les en-têtes des fichiers de `corpus/` et cités par les sources publiques recensées par `survey-sas7bdat-prior-art`
- **THEN** chacun de ces codes MUST figurer dans la table avec un nom SAS et un encodage standard, ou être explicitement marqué sans équivalent standard

#### Scenario: Encodage sans équivalent WHATWG
- **WHEN** un encodage SAS n'a pas d'équivalent dans le standard WHATWG
- **THEN** la table MUST donner le nom IANA le plus proche et signaler l'écart à prendre en charge par le driver

#### Scenario: Vérification par le vérificateur d'encodage
- **WHEN** le vérificateur d'encodage de `rea_output/tools/` décode les colonnes caractère des fichiers du corpus avec l'encodage déduit de la table
- **THEN** le texte obtenu MUST être identique à celui affiché par l'oracle SAS pour ces fichiers

### Requirement: Comportement sur octets invalides et troncature multi-octets
Le livrable SHALL décrire le comportement de SAS lorsqu'une valeur caractère contient des octets invalides pour l'encodage du fichier, et lorsqu'un caractère multi-octets est tronqué en fin de champ, afin que le driver puisse reproduire ou signaler ces cas.

#### Scenario: Octets invalides
- **WHEN** l'oracle lit un fichier du corpus contenant des octets invalides pour son encodage déclaré
- **THEN** la spécification MUST décrire le résultat observé (substitution, erreur, conservation brute) avec la preuve runtime correspondante

#### Scenario: Caractère multi-octets tronqué en fin de champ
- **WHEN** l'oracle lit un champ caractère dont le dernier caractère multi-octets est coupé par la largeur de la colonne
- **THEN** la spécification MUST décrire le résultat observé, et tout comportement non déterminé MUST être consigné dans `rea_output/60_inconnues.md`

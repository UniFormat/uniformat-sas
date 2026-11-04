## Purpose

Spécifier, pour le futur driver Rust, comment interpréter les noms de format attachés aux colonnes d'un `.sas7bdat` (type logique et type Arrow cible), comment afficher une valeur comme SAS pour les formats courants, et comment lire les formats utilisateur stockés dans les catalogues `.sas7bcat`.

## ADDED Requirements

### Requirement: Inventaire des formats et informats natifs
Le livrable `rea_output/30_formats_encodages.md` SHALL contenir l'inventaire de tous les formats et informats natifs connus de `src/sasexe/tkefmt.so` et des modules `tkefmf*.so` / `tkefmi*.so` (tables de noms, chaînes, symboles), en indiquant pour chaque famille le module qui la fournit et le niveau de confiance (`confirmé`, `inféré`, `inconnu`) de chaque entrée.

#### Scenario: Chaque format inventorié est rattaché à un module
- **WHEN** un relecteur parcourt l'inventaire des formats et informats natifs
- **THEN** chaque entrée MUST indiquer son nom, s'il s'agit d'un format ou d'un informat, son module d'origine (`tkefmt.so`, `tkefmf*.so` ou `tkefmi*.so`), la preuve (fonction@adresse, chaîne ou symbole) et un niveau de confiance

#### Scenario: Formats NLS et localisés listés sans spécification
- **WHEN** l'inventaire rencontre un format NLS (`NLDATE`…) ou localisé
- **THEN** ce format MUST être listé dans une section dédiée marquée hors priorité, sans algorithme d'affichage spécifié

### Requirement: Table de classification format vers type Arrow
Le livrable `rea_output/30_formats_encodages.md` SHALL commencer par une table de classification couvrant tous les formats natifs inventoriés, associant à chacun une catégorie logique et un type Arrow cible : date (jours depuis 1960-01-01), datetime (secondes depuis 1960-01-01T00:00:00), heure (secondes), numérique, décimal à échelle fixe, caractère. Les formats ambigus MUST être signalés avec la raison de l'ambiguïté.

#### Scenario: Classification complète
- **WHEN** on compare la liste des formats natifs inventoriés à la table de classification
- **THEN** chaque format inventorié MUST apparaître dans la table avec une catégorie logique et un type Arrow cible, ou être explicitement marqué ambigu

#### Scenario: Époque et unité explicites
- **WHEN** un format est classé date, datetime ou heure
- **THEN** la table MUST préciser l'époque (1960-01-01 ou 1960-01-01T00:00:00) et l'unité (jours ou secondes) de la valeur stockée

#### Scenario: Format ambigu signalé
- **WHEN** un format peut s'appliquer à plusieurs catégories logiques selon la valeur ou le contexte
- **THEN** la table MUST le marquer ambigu et indiquer la règle de repli proposée au driver

### Requirement: Algorithmes d'affichage des formats courants
Le livrable SHALL spécifier l'algorithme d'affichage des formats `w.d`, `BEST`, `E`, `COMMA`, `DOLLAR`, `PERCENT`, `Z`, `DATE`, `DATETIME`, `TIME`, `YYMMDD`, `DDMMYY`, `MMDDYY`, ISO 8601 (`E8601DA`, `E8601DT`…), `$CHAR` et `$`, y compris l'arrondi, le comportement en largeur insuffisante (`*`, bascule en notation E) et l'affichage des valeurs manquantes.

#### Scenario: Reproduction de PROC PRINT sur le corpus
- **WHEN** le vérificateur d'affichage de `rea_output/tools/` formate toutes les valeurs du corpus portant un des formats de cette liste
- **THEN** sa sortie MUST être identique, caractère pour caractère, à la sortie `PROC PRINT` de l'oracle SAS pour ces mêmes valeurs

#### Scenario: Largeur insuffisante
- **WHEN** une valeur ne tient pas dans la largeur `w` du format
- **THEN** la spécification MUST décrire le résultat produit par SAS (astérisques, notation E ou réduction de décimales) et le corpus MUST contenir au moins un cas vérifié

#### Scenario: Valeurs manquantes
- **WHEN** une valeur manquante numérique (`.`, `._`, `.A` à `.Z`) ou caractère est affichée avec un format de la liste
- **THEN** la spécification MUST décrire sa représentation affichée, vérifiée contre `PROC PRINT`

### Requirement: Structure des catalogues de formats utilisateur
Le livrable SHALL spécifier la structure d'un catalogue `.sas7bcat` de formats utilisateur — en-tête, répertoire des entrées, entrées `FORMAT`, `FORMATC`, `INFMT`, `INFMTC`, plages, libellés, clause `OTHER`, `PICTURE` — selon le modèle d'entrée de spécification de `prompts/README.md`, à partir de catalogues générés par `PROC FORMAT` dans l'oracle ou, à défaut, de catalogues publics de `corpus/external/`. `src/sashelp/core.sas7bcat` MUST être utilisé uniquement pour la structure générique d'un catalogue, jamais comme exemple de formats utilisateur.

#### Scenario: Lecteur de référence équivalent à CNTLOUT=
- **WHEN** le lecteur de référence de `rea_output/tools/` lit au moins un catalogue du corpus contenant des formats `VALUE`, `$VALUE`, `INVALUE` et `PICTURE`
- **THEN** les formats restitués (noms, plages, bornes, libellés, options) MUST être identiques à la sortie `CNTLOUT=` produite par SAS pour ce catalogue

#### Scenario: Partie bloquée
- **WHEN** `PROC FORMAT` ne fonctionne pas dans l'oracle et qu'aucun catalogue public exploitable n'existe dans `corpus/external/`
- **THEN** la partie B du livrable MUST être explicitement marquée bloquée avec la raison, et le blocage MUST être consigné dans `rea_output/STATE.md`

#### Scenario: Octets non expliqués
- **WHEN** une zone d'octets d'un catalogue reste non expliquée
- **THEN** elle MUST être consignée dans `rea_output/60_inconnues.md` avec son emplacement, et MUST NOT être masquée

### Requirement: Preuves et traçabilité de l'analyse
Toute affirmation du livrable SHALL porter un niveau de confiance et une preuve (binaire, fichier du corpus ou sortie runtime) archivée sous `rea_output/evidence/` ; les spécifications publiques (ReadStat, Parso, pandas, Shotwell) MUST être traitées comme des hypothèses à vérifier, jamais comme des preuves.

#### Scenario: Affirmation confirmée
- **WHEN** une affirmation est marquée `confirmé`
- **THEN** elle MUST citer à la fois une preuve binaire et une preuve fichier ou runtime présentes dans `rea_output/evidence/`

#### Scenario: Hypothèse publique non vérifiée
- **WHEN** une information provient uniquement d'une source publique
- **THEN** elle MUST être marquée `inféré` ou `inconnu`, jamais `confirmé`

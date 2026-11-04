## Purpose

Fournir un corpus de fichiers `.sas7bdat` de référence, généré par familles en ne faisant varier qu'un seul paramètre à la fois, accompagné de sorties de contrôle et d'un manifeste vérifiable, pour permettre l'analyse différentielle du format et la validation du futur driver.

## ADDED Requirements

### Requirement: Programmes de génération par famille
Le corpus SHALL contenir, sous `corpus/gen/`, un programme SAS (`*.sas`) par famille et un script unique qui les exécute tous via la commande documentée de l'oracle. Les familles minimales MUST être :
- base : 0, 1, 2, 100 et 3 000 lignes ; 1 num ; 1 char ; mélange ;
- types : longueurs num 3 à 8 ; longueurs char 1, 8, 200, 32 767 ;
- valeurs : missing `.`, `._`, `.A` à `.Z`, infinis/NaN si SAS en produit, valeurs extrêmes ;
- métadonnées : noms de 1 et 32 caractères, labels vides / 256 caractères / non ASCII, label de table, formats et informats variés (`date9.`, `datetime19.`, `time8.`, `dollar12.2`, `best12.`, `comma10.2`, `percent8.1`, `$char20.`, `e10.`) ;
- taille : assez de lignes ou de colonnes pour forcer plusieurs pages, et assez de colonnes pour produire plus d'un sous-en-tête de noms ;
- compression : chaque table en `NO`, `CHAR`, `BINARY` ;
- suppression : lignes supprimées, si l'oracle le permet ;
- encodage : UTF-8, et latin1/wlatin1 si possible.

#### Scenario: Régénération complète
- **WHEN** le script de `corpus/gen/` est exécuté sur l'oracle
- **THEN** tous les fichiers générés du corpus sont reproduits sans `ERROR` dans les logs

#### Scenario: Famille non couverte
- **WHEN** une famille minimale n'a pas de programme dans `corpus/gen/`
- **THEN** `corpus/MANIFEST.md` contient une ligne qui nomme la famille et explique pourquoi elle manque

### Requirement: Un seul paramètre varié à la fois
Chaque fichier généré MUST différer de son fichier de référence dans la même famille par un seul paramètre, et ce paramètre SHALL être nommé dans `corpus/MANIFEST.md`.

#### Scenario: Paire comparable
- **WHEN** deux fichiers d'une même famille sont comparés
- **THEN** le manifeste permet d'identifier l'unique paramètre qui les distingue (par exemple le mode de compression ou la longueur d'une variable char)

### Requirement: Variantes de compression par famille
Chaque famille du corpus SHALL disposer de ses variantes `COMPRESS=NO`, `CHAR` et `BINARY`, ou d'une ligne dans `corpus/MANIFEST.md` expliquant pourquoi une variante manque.

#### Scenario: Critère de fin sur la compression
- **WHEN** le corpus est déclaré terminé
- **THEN** pour chaque famille, les trois variantes `NO` / `CHAR` / `BINARY` existent, ou chaque variante absente est justifiée dans `corpus/MANIFEST.md`

### Requirement: Sorties de contrôle par table
Pour chaque table générée, le corpus SHALL contenir le `.sas7bdat`, la sortie texte de `PROC CONTENTS`, la sortie de `PROC PRINT` et un export CSV de référence. Si `PROC EXPORT` est indisponible, le CSV MUST être produit par une étape DATA (`put` avec `best32.` pour les numériques, code de missing spécial écrit tel quel).

#### Scenario: Table complète
- **WHEN** une table est générée
- **THEN** ses quatre fichiers (`.sas7bdat`, CONTENTS, PRINT, CSV) existent côte à côte dans `corpus/`

#### Scenario: Missing spéciaux dans le CSV
- **WHEN** une table de la famille valeurs contient `.A` ou `._`
- **THEN** le CSV de référence contient le code de missing tel qu'affiché par SAS, et non une cellule vide

### Requirement: Fichiers externes pour les cas hors oracle
Les cas impossibles à produire avec l'oracle (big endian, 32 bits, chiffrement, autres OS) MUST NOT être fabriqués. Ils SHALL être couverts, quand c'est possible, par des fichiers d'exemple publics (dépôts de tests de ReadStat, Parso, pandas) placés dans `corpus/external/`, avec source et licence notées.

#### Scenario: Fichier externe ajouté
- **WHEN** un fichier public est copié dans `corpus/external/`
- **THEN** sa ligne de manifeste indique l'URL ou le dépôt source, la licence et le cas couvert

#### Scenario: Cas sans fichier disponible
- **WHEN** aucun fichier public n'est trouvé pour un cas hors oracle
- **THEN** le cas est listé comme manquant dans `corpus/MANIFEST.md`, sans fichier inventé

### Requirement: Manifeste du corpus
Le fichier `corpus/MANIFEST.md` SHALL contenir une ligne par fichier du corpus avec : nom, famille, paramètre varié, source (générée ou externe), outil de génération, taille en octets et empreinte SHA-256.

#### Scenario: Intégrité vérifiable
- **WHEN** l'empreinte SHA-256 d'un fichier du corpus est recalculée
- **THEN** elle correspond à celle de `corpus/MANIFEST.md`

#### Scenario: Fichier non référencé
- **WHEN** un fichier présent dans `corpus/` n'a pas de ligne dans le manifeste
- **THEN** le corpus est considéré incomplet jusqu'à l'ajout de cette ligne

### Requirement: Aucune analyse des octets
Ce corpus MUST être produit sans analyse des octets ni décompilation ; l'interprétation du contenu binaire relève des changements `specify-sas7bdat-read-format`, `specify-sas7bdat-compression`, `specify-sas-formats-encodings` et `specify-sas7bdat-writing`.

#### Scenario: Livrables limités
- **WHEN** le changement est terminé
- **THEN** aucun document d'analyse de format (`rea_output/1*.md`, `2*.md`, `3*.md`) ni sortie de décompilation n'a été produit par ce changement

### Requirement: Mise à jour de l'état de session
En fin de travail, `rea_output/STATE.md` SHALL être mis à jour selon le format commun : dernière session (prompt 01, date, résultat), avancement, blocages ouverts, décisions prises (environnement retenu), capacités testées de l'oracle et prochaine action.

#### Scenario: Fin de session
- **WHEN** la session du prompt 01 se termine, terminée, partielle ou bloquée
- **THEN** `rea_output/STATE.md` reflète ce résultat et indique la prochaine action

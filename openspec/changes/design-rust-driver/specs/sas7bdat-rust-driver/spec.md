## Purpose

Définir la conception du driver Rust `sas7bdat` (lecture, écriture, Arrow, Python, R, SQL DataFusion, puis Arrow Flight), établie à partir de la seule spécification du format, sous forme d'architecture, d'ADR, de stratégie de tests et de feuille de route vérifiables, sans aucun code d'implémentation.

## ADDED Requirements

### Requirement: Entrées limitées aux documents de spécification
La session de conception SHALL lire uniquement `prompts/README.md`, `rea_output/STATE.md`, `rea_output/00_etat_de_l_art.md`, `rea_output/10_format_sas7bdat.md`, `rea_output/15_compression.md`, `rea_output/20_ecriture.md`, `rea_output/30_formats_encodages.md`, `rea_output/60_inconnues.md` et `corpus/MANIFEST.md`. Elle MUST NOT lire `src/`, `rea_output/evidence/` ni aucune sortie de décompilation, et MUST NOT utiliser la skill REA. Le code source d'une implémentation existante (ReadStat, Parso, pandas…) MUST NOT être lu sauf si la section licences de `rea_output/00_etat_de_l_art.md` l'autorise.

#### Scenario: Entrées tracées
- **WHEN** la conception est terminée
- **THEN** `docs/architecture.md` liste les documents d'entrée lus, tous appartenant à la liste autorisée

#### Scenario: Demande de lecture interdite
- **WHEN** une information semble manquer et ne se trouverait que dans `src/`, `rea_output/evidence/` ou une sortie de décompilation
- **THEN** le manque est consigné comme question ouverte (renvoi vers le changement de spécification concerné) au lieu d'être comblé par cette lecture

#### Scenario: Prérequis de spécification absents
- **WHEN** l'un des documents `10`, `15`, `20`, `30` ou `60` n'existe pas dans `rea_output/`
- **THEN** la session s'arrête, le blocage est consigné dans `rea_output/STATE.md` avec le changement amont à terminer, et aucun livrable partiel n'est présenté comme complet

### Requirement: Document d'architecture
Le changement SHALL produire `docs/architecture.md` contenant une vue d'ensemble, le découpage en crates, le flux de données (fichier → pages → lignes → lots Arrow → consommateurs) et une API publique esquissée sous forme de signatures, sans corps d'implémentation. Il MUST couvrir les cinq usages : bibliothèque Rust lecture/écriture avec métadonnées complètes (labels, formats, informats, encodage, horodatages, compression, nom et label de table) ; sortie Apache Arrow en streaming par lots de lignes par groupe de pages avec schéma disponible sans lire les données ; bindings Python (PyO3 + maturin ; pyarrow, polars, pandas) et R (extendr ; attributs compatibles `haven` : `label`, `format.sas`, missing étiquetés) ; SQL via DataFusion (`TableProvider`, projection et `LIMIT` poussées dans la lecture, lecture parallèle par plage de pages, statistiques sans tout lire) ; source Arrow Flight ou connecteur type Dremio (lecture distribuée par plages de pages, schéma exposé seul).

#### Scenario: Usages couverts
- **WHEN** on relit `docs/architecture.md`
- **THEN** chacun des cinq usages est rattaché à au moins une crate et à une esquisse d'API publique

#### Scenario: Schéma sans données
- **WHEN** on cherche dans l'API esquissée comment obtenir le schéma Arrow d'un fichier
- **THEN** une signature permet de l'obtenir à partir des seuls en-têtes et sous-en-têtes, sans lire les pages de données

### Requirement: Traçabilité spécification vers code
Chaque section des documents `rea_output/10_format_sas7bdat.md`, `15_compression.md`, `20_ecriture.md` et `30_formats_encodages.md` SHALL être rattachée à un module ou une crate dans `docs/architecture.md` (table de traçabilité).

#### Scenario: Section non rattachée
- **WHEN** une section de `10`, `15`, `20` ou `30` n'apparaît dans aucune ligne de la table de traçabilité
- **THEN** le critère de fin n'est pas atteint et la conception est complétée avant clôture

### Requirement: Une ADR par décision
Le changement SHALL produire une ADR par décision dans `docs/adr/NNNN-*.md` (numérotation séquentielle), chacune avec contexte, options étudiées, décision et conséquences. Les décisions MUST inclure au minimum : découpage en crates et dépendances autorisées dans le cœur ; modèle d'I/O (`Read + Seek`, mmap, `object_store` S3/HTTP, async ou non) ; mapping des types ; sémantique SQL ; API et règles d'écriture ; gestion des inconnus ; erreurs (`thiserror`), politique `unsafe` (interdit ou circonscrit), MSRV et licence du driver.

#### Scenario: Liste des ADR complète
- **WHEN** on liste `docs/adr/`
- **THEN** chacune des sept décisions ci-dessus a sa propre ADR, et chaque ADR documente au moins deux options étudiées

#### Scenario: Licence alignée sur l'état de l'art
- **WHEN** on relit l'ADR erreurs/`unsafe`/MSRV/licence
- **THEN** le choix de licence est justifié par la section licences de `rea_output/00_etat_de_l_art.md`

### Requirement: Cœur sans I/O
Le découpage SHALL isoler un crate cœur `sas7bdat` qui décode et encode le format à partir de tampons d'octets, sans dépendance d'I/O, d'async ni d'Arrow ; les adaptateurs (`-arrow`, `-datafusion`, `-py`, `-r`, `-cli`) MUST dépendre du cœur et non l'inverse. L'ADR de découpage MUST lister les dépendances autorisées dans le cœur.

#### Scenario: Graphe de dépendances
- **WHEN** on relit le graphe de crates de `docs/architecture.md`
- **THEN** aucune flèche ne part du cœur vers un adaptateur, et les dépendances du cœur sont toutes dans la liste autorisée de l'ADR

### Requirement: Mapping des types et missing spéciaux
L'ADR de mapping SHALL dériver le type Arrow de chaque variable de la table de classification de `rea_output/30_formats_encodages.md` : numérique → Float64, Date32, Timestamp, Time64 ou Decimal selon le format ; caractère → Utf8 avec option documentée de conservation ou de retrait des blancs finaux ; missing → null. Elle MUST choisir et justifier un mécanisme de conservation du code de missing spécial (`.A`–`.Z`, `._`) parmi colonne annexe, métadonnée Arrow ou type extension, et préciser comment Python et R (missing étiquetés `haven`) le restituent.

#### Scenario: Format de date
- **WHEN** une variable numérique porte un format classé « date » dans la table de `30`
- **THEN** l'ADR indique le type Arrow cible (Date32) et la conversion depuis l'époque SAS

#### Scenario: Missing spécial conservé
- **WHEN** une valeur vaut `.B`
- **THEN** l'ADR décrit comment elle devient null en Arrow tout en restant distinguable de `.` et de `.C`, et comment elle est restituée en R et en Python

### Requirement: Sémantique SQL du driver
L'ADR de sémantique SQL SHALL fixer, comme choix de conception du driver (et non comme résultat de reverse engineering) : la comparaison de chaînes avec espaces finaux, l'ordre de tri des missing (y compris spéciaux) par rapport aux valeurs, et l'invisibilité des lignes supprimées. Elle MUST aussi préciser comment projection, `LIMIT`, lecture parallèle par plages de pages et statistiques sont exposées à DataFusion.

#### Scenario: Ligne supprimée
- **WHEN** un fichier contient une ligne marquée supprimée selon `10`
- **THEN** l'ADR spécifie qu'elle n'apparaît dans aucun résultat de requête ni dans les statistiques de nombre de lignes

#### Scenario: Comparaison avec blancs finaux
- **WHEN** une requête compare `'abc'` à une valeur stockée `'abc  '`
- **THEN** l'ADR spécifie le résultat et sa cohérence avec l'option de retrait des blancs finaux

### Requirement: Conception de l'écriture
L'ADR d'écriture SHALL définir l'API d'écriture depuis des lots Arrow et depuis un itérateur de lignes, le choix de compression (aucune, `CHAR`, `BINARY`), la validation des limites listées dans `rea_output/20_ecriture.md` avant toute écriture, et le comportement face à une valeur non représentable (erreur, troncature signalée ou conversion), sans produire de fichier partiel silencieusement invalide.

#### Scenario: Limite dépassée
- **WHEN** un schéma dépasse une limite de `20` (longueur de nom, nombre de variables, longueur de caractère…)
- **THEN** l'API esquissée renvoie une erreur typée avant l'écriture du premier octet

#### Scenario: Valeur non représentable
- **WHEN** une valeur ne peut pas être encodée dans l'encodage ou le type cible
- **THEN** l'ADR définit un comportement unique et configurable, documenté dans l'API

### Requirement: Comportement défini pour chaque inconnu
L'ADR de gestion des inconnus SHALL attribuer à chaque entrée de `rea_output/60_inconnues.md` un comportement du lecteur et de l'écrivain : erreur, avertissement ou tolérance (avec valeur écrite par défaut le cas échéant).

#### Scenario: Couverture des inconnus
- **WHEN** on compare `rea_output/60_inconnues.md` à la table de l'ADR
- **THEN** chaque inconnu y figure avec un comportement en lecture et en écriture

### Requirement: Stratégie de tests
`docs/architecture.md` (ou une section dédiée) SHALL décrire la stratégie de tests : corpus `corpus/` comme tests d'intégration comparés aux CSV de référence ; round-trip écriture → lecture par le driver, SAS, ReadStat et pandas ; tests par propriétés (proptest) sur le décodage des valeurs et la compression ; fuzzing `cargo-fuzz` du parseur d'en-tête, de pages et du décompresseur avec pour objectif aucune panique et aucune allocation non bornée sur fichier hostile ; benchmarks contre ReadStat/pyreadstat et pandas sur le même corpus.

#### Scenario: Fichier hostile
- **WHEN** on relit la stratégie de fuzzing
- **THEN** elle nomme les cibles (en-tête, pages, décompresseur) et les propriétés vérifiées (aucune panique, allocations bornées par la taille du fichier ou une limite configurée)

### Requirement: Feuille de route à jalons
Le changement SHALL produire `docs/roadmap.md` avec les jalons dans l'ordre : lecture non compressée → compression → métadonnées/formats → Arrow/DataFusion → Python/R → écriture → Flight, chacun avec un critère de fin vérifiable.

#### Scenario: Critère de fin par jalon
- **WHEN** on relit `docs/roadmap.md`
- **THEN** les sept jalons apparaissent dans cet ordre et chacun a un critère de fin mesurable (tests du corpus, round-trip, etc.)

### Requirement: Aucun code d'implémentation
La session de conception MUST NOT écrire de code d'implémentation : aucun `Cargo.toml`, aucun fichier `.rs`, `.py` ou `.R` exécutable ; seules des signatures dans les documents Markdown sont admises.

#### Scenario: Vérification des fichiers produits
- **WHEN** la session se termine
- **THEN** les seuls fichiers créés ou modifiés sont `docs/architecture.md`, `docs/adr/*.md`, `docs/roadmap.md` et `rea_output/STATE.md`

### Requirement: Mise à jour de l'état
La session SHALL mettre à jour `rea_output/STATE.md` en dernier, selon le format de `prompts/README.md` : dernière session (prompt 06, date, résultat), avancement, blocages ouverts, décisions prises (une ligne par ADR) et prochaine action.

#### Scenario: Clôture de session
- **WHEN** les livrables sont écrits
- **THEN** `rea_output/STATE.md` indique le prompt 06 avec son statut et si le critère de fin est atteint

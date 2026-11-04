## Purpose

Fournir une spécification complète et vérifiée de la structure d'un fichier `.sas7bdat` non compressé, assez précise pour écrire un lecteur indépendant de SAS et servir de base aux spécifications de compression, d'encodage, d'écriture et à la conception du driver Rust.

## ADDED Requirements

### Requirement: Prérequis avant toute analyse binaire
Le travail SHALL commencer par la lecture de `prompts/README.md`, `rea_output/STATE.md`, `rea_output/00_etat_de_l_art.md` (livrable de `survey-sas7bdat-prior-art`) et `corpus/MANIFEST.md` (livrable de `setup-sas-oracle-corpus`). La décompilation des binaires SAS MUST NOT être entreprise tant que `rea_output/STATE.md` ne consigne pas la confirmation interne NULL (référent licence SAS / juridique) que l'analyse à des fins d'interopérabilité est couverte.

#### Scenario: Confirmation de licence consignée
- **WHEN** `rea_output/STATE.md` contient la confirmation interne datée et sa source (référent licence SAS ou juridique NULL)
- **THEN** l'étape de décompilation ciblée peut être engagée

#### Scenario: Confirmation absente
- **WHEN** la confirmation n'est pas consignée dans `rea_output/STATE.md`
- **THEN** seules les étapes sans décompilation (état de l'art, analyse différentielle) sont menées, et le point est inscrit dans « Blocages ouverts » de `STATE.md`

#### Scenario: Dépendances non livrées
- **WHEN** `rea_output/00_etat_de_l_art.md` ou `corpus/MANIFEST.md` est absent
- **THEN** la session s'arrête sans produire la spec et le signale dans `STATE.md`

### Requirement: Format d'entrée de la spécification
La spécification `rea_output/10_format_sas7bdat.md` SHALL être rédigée en français et décrire chaque champ binaire selon le modèle d'entrée de `prompts/README.md` : emplacement (structure + offset 32 bits / 64 bits), taille et type, sémantique, valeurs observées (avec fichiers du corpus), règle d'écriture, preuve, et niveau de confiance `confirmé`, `inféré` ou `inconnu`. Une entrée `confirmé` MUST citer à la fois une preuve binaire (fonction@adresse) et une preuve fichier ou runtime.

#### Scenario: Entrée conforme
- **WHEN** un relecteur examine une entrée de champ de la spec
- **THEN** les sept rubriques du modèle sont présentes et la confiance est l'une des trois valeurs autorisées

#### Scenario: Confiance confirmée sans double preuve
- **WHEN** une entrée ne cite qu'une seule source de preuve
- **THEN** sa confiance MUST être `inféré` et non `confirmé`

### Requirement: Méthode ordonnée de production
La spec SHALL être produite dans cet ordre : (1) reprise de la structure de `00_etat_de_l_art.md` comme hypothèse de départ, champ par champ, en `inféré` ; (2) analyse différentielle des paires du corpus ne différant que par un paramètre ; (3) décompilation ciblée limitée aux champs encore `inféré` ou `inconnu` et aux règles de validation à la lecture ; (4) vérification par un lecteur de référence. Les spécifications publiques (ReadStat, Parso, pandas, Shotwell) MUST être traitées comme des hypothèses, jamais comme des preuves.

#### Scenario: Champ établi par différentiel
- **WHEN** un champ est confirmé ou corrigé par l'analyse différentielle
- **THEN** il n'est pas soumis à la décompilation, et la preuve cite la paire de fichiers et l'offset

#### Scenario: Champ resté incertain
- **WHEN** un champ reste `inféré` ou `inconnu` après l'analyse différentielle
- **THEN** il est ciblé par la décompilation dans les modules `sase7io`, `sase7opn`, `sase7l`, `sase7ld`, `sase7lu`, `sase7xgt`, `sase7xop`, `sase7xrt`, `src/dbcs/sasexe/sase7`, `sase7yse`

### Requirement: Outils de comparaison réutilisables
Les scripts d'analyse différentielle (hexdump, diff, extraction de champs) SHALL être écrits dans `rea_output/tools/` sous forme réutilisable et relançable, et non exécutés à la volée.

#### Scenario: Rejouer une comparaison
- **WHEN** un relecteur relance un script de `rea_output/tools/` sur une paire du corpus
- **THEN** il obtient la même comparaison que celle citée en preuve dans la spec

### Requirement: Décompilation ciblée et non recopiante
Les preuves issues de la décompilation SHALL noter la fonction et l'adresse et décrire le comportement observé. La spec MUST NOT contenir de pseudo-code ni de code SAS recopié. `src/` MUST rester en lecture seule.

#### Scenario: Preuve binaire citée
- **WHEN** un champ est établi par décompilation
- **THEN** la rubrique Preuve cite `<fonction>@<adresse>` et la sémantique est décrite en prose

#### Scenario: Pseudo-code détecté
- **WHEN** une entrée de la spec reproduit du pseudo-code décompilé
- **THEN** l'entrée est réécrite en description de comportement avant clôture

### Requirement: Couverture de l'en-tête de fichier
La spec SHALL décrire tous les champs de l'en-tête : magic number, alignements 32/64 bits, endianness, plateforme, code d'encodage (correspondance code → nom), horodatages (epoch 1960), nom et type de table, versions SAS et OS, taille d'en-tête, taille de page, nombre de pages, et tous les champs restants, y compris ceux dits « réservés ».

#### Scenario: En-tête entièrement couvert
- **WHEN** on parcourt l'en-tête octet par octet pour les variantes 32 et 64 bits
- **THEN** chaque octet appartient à une entrée de la spec ou à une entrée de `rea_output/60_inconnues.md`

### Requirement: Couverture des pages
La spec SHALL décrire les types de pages (meta, data, mix, amd et tout autre type rencontré), l'en-tête de page, les pointeurs de sous-en-têtes, la position des lignes dans les pages data et mix, ainsi que les règles de padding et d'alignement.

#### Scenario: Localisation des lignes
- **WHEN** un lecteur applique la spec à une page data ou mix
- **THEN** il détermine sans ambiguïté le nombre de lignes de la page et l'offset de chacune

### Requirement: Couverture des sous-en-têtes
La spec SHALL décrire les signatures des sous-en-têtes par variante (32/64 bits, LE/BE) et le contenu de : row size, column size, subheader counts, column text, column names, column attributes, format and label, column list, ainsi que tout sous-en-tête inconnu rencontré dans le corpus.

#### Scenario: Sous-en-tête inconnu rencontré
- **WHEN** une signature de sous-en-tête du corpus n'est pas identifiée
- **THEN** elle est consignée avec ses occurrences dans la spec et dans `rea_output/60_inconnues.md`

### Requirement: Couverture des colonnes
La spec SHALL décrire, pour chaque colonne, le type, la longueur, l'offset dans la ligne, le nom, le label, et le format et l'informat (nom, largeur, décimales) obtenus via les références au texte (column text).

#### Scenario: Métadonnées de colonnes reconstituées
- **WHEN** un lecteur applique la spec à un fichier du corpus
- **THEN** il retrouve pour chaque colonne les mêmes nom, type, longueur, label et format que `PROC CONTENTS` dans les sorties de référence du corpus

### Requirement: Couverture des valeurs et des lignes supprimées
La spec SHALL décrire les numériques tronqués (3 à 8 octets), les valeurs manquantes standard et spéciales (`.A`–`.Z`, `._`), les valeurs caractères (padding, code d'encodage de stockage), ainsi que la représentation des lignes supprimées et la règle de lecture associée.

#### Scenario: Numérique tronqué
- **WHEN** une colonne numérique a une longueur inférieure à 8 octets
- **THEN** la spec indique comment reconstituer la valeur flottante complète

#### Scenario: Ligne supprimée
- **WHEN** un fichier du corpus contient des lignes supprimées
- **THEN** la spec indique comment les détecter et le lecteur de référence les exclut comme SAS

### Requirement: Contrôles effectués par SAS à l'ouverture
La spec SHALL lister les vérifications que SAS applique à l'ouverture d'un `.sas7bdat` (champs contrôlés, erreurs déclenchées, messages correspondants de `src/sasmsg/`), afin de servir de contrainte à l'écriture (`specify-sas7bdat-writing`).

#### Scenario: Contrôle documenté
- **WHEN** une règle de validation est identifiée
- **THEN** la spec indique le champ concerné, la condition de rejet et le message SAS associé

### Requirement: Lecteur de référence indépendant
Un lecteur Python minimal `rea_output/tools/ref_reader.py` SHALL être écrit à partir de la seule spec, sans dépendance à ReadStat, pandas ni Parso. Tout écart constaté avec le CSV de référence MUST entraîner une correction de la spec, et pas seulement du script.

#### Scenario: Lecture de tout le corpus non compressé
- **WHEN** `ref_reader.py` lit tous les fichiers non compressés du corpus (générés et `external/`, au moins en LE/64 bits)
- **THEN** le résultat est identique au CSV de référence pour chaque fichier

#### Scenario: Écart détecté
- **WHEN** la sortie de `ref_reader.py` diffère du CSV de référence
- **THEN** la cause est corrigée dans `rea_output/10_format_sas7bdat.md` avant de corriger le script

### Requirement: Aucun inconnu masqué
Toute zone d'octets de l'en-tête ou des sous-en-têtes non expliquée SHALL être consignée dans `rea_output/60_inconnues.md` (et via `record_unknown` dans REA). La spec MUST NOT omettre un champ.

#### Scenario: Complétude octet par octet
- **WHEN** on vérifie la couverture de l'en-tête et de chaque sous-en-tête
- **THEN** chaque octet est décrit dans la spec ou listé dans `60_inconnues.md`

### Requirement: Preuves archivées et état mis à jour
Les preuves SHALL être déposées dans `rea_output/evidence/` (bundle REA exporté par `export_evidence_bundle`, hexdumps annotés), et `rea_output/STATE.md` MUST être mis à jour en fin de session selon le format de `prompts/README.md` (dernière session, avancement, blocages, décisions, prochaine action).

#### Scenario: Clôture de session
- **WHEN** la session se termine, que le critère de fin soit atteint ou non
- **THEN** `STATE.md` indique le statut (terminé, partiel ou bloqué), si le critère de fin est atteint, et la prochaine action

### Requirement: Périmètre limité au non compressé
La spec SHALL se limiter aux fichiers non compressés et au seul code d'encodage. La compression (`specify-sas7bdat-compression`), les formats et encodages au-delà du code (`specify-sas-formats-encodings`) et l'écriture (`specify-sas7bdat-writing`) MUST NOT y être spécifiés, hormis par renvoi.

#### Scenario: Fichier compressé rencontré
- **WHEN** un fichier compressé du corpus est examiné
- **THEN** seuls les éléments de structure communs sont décrits, et la compression est renvoyée à `specify-sas7bdat-compression`

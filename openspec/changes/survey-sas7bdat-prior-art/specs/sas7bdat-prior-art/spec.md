## Purpose

Établir, à partir des seules sources publiques, ce qui est déjà connu du format `.sas7bdat` (implémentations, documentations, capacités, désaccords, licences), afin que le reverse engineering des changements 02 à 05 ne porte que sur les manques réels et que la licence du futur driver Rust soit choisie en connaissance de cause.

## ADDED Requirements

### Requirement: Document d'état de l'art structuré
Le changement SHALL produire le fichier `rea_output/00_etat_de_l_art.md`, rédigé en français, contenant exactement cinq sections numérotées : 1. Inventaire, 2. Matrice des capacités, 3. Points de désaccord, 4. Liste des manques, 5. Contraintes de licence.

#### Scenario: Présence des cinq sections
- **WHEN** on ouvre `rea_output/00_etat_de_l_art.md` à la fin du changement
- **THEN** le fichier existe et contient les sections 1 à 5 dans cet ordre, chacune non vide

### Requirement: Inventaire sourcé des implémentations et documentations
La section 1 MUST recenser au minimum : ReadStat (C, utilisé par `haven` et `pyreadstat`), Parso (Java), pandas (`pandas.io.sas`), le paquet Python `sas7bdat`, le PDF de Matthew Shotwell (paquet R `sas7bdat`), `readstat-rs` et toute autre crate Rust publiée sur crates.io qui lit ou écrit le format, les implémentations Go ou JavaScript pertinentes, et la documentation SAS officielle publique (options `COMPRESS=`, `ENCODING=`, formats). Pour chaque entrée, elle MUST indiquer l'URL, la dernière version ou date d'activité constatée, et la licence telle que lue sur la source (fichier `LICENSE`, en-tête ou métadonnées du registre), avec la date de consultation.

#### Scenario: Entrée complète
- **WHEN** on lit une entrée quelconque de l'inventaire
- **THEN** elle comporte une URL, une version ou date d'activité, une licence et la référence du fichier ou de la page où cette licence a été lue

#### Scenario: Implémentation absente ou introuvable
- **WHEN** une implémentation de la liste minimale ne peut pas être localisée ou n'est plus publiée
- **THEN** l'inventaire la mentionne quand même, avec le constat explicite (introuvable, archivée, retirée) et la recherche effectuée

### Requirement: Licences vérifiées sur la source
Toute licence citée dans le document MUST provenir d'une lecture de la source publique au moment du travail ; une licence citée de mémoire ou non vérifiable SHALL être marquée `inconnu` au lieu d'être affirmée.

#### Scenario: Licence non vérifiable
- **WHEN** aucune déclaration de licence n'est trouvée dans le dépôt ni dans les métadonnées du registre
- **THEN** la licence est notée `inconnu` avec la mention des emplacements consultés, et la source est traitée comme non réutilisable dans la section 5

### Requirement: Matrice des capacités entièrement sourcée
La section 2 MUST présenter une matrice avec une ligne par capacité et une colonne par implémentation inventoriée. Les lignes SHALL couvrir au minimum : lecture ; écriture ; 32 vs 64 bits ; little vs big endian ; compression `CHAR` (RLE) en lecture ; `CHAR` en écriture ; compression `BINARY` (RDC) en lecture ; `BINARY` en écriture ; lignes supprimées ; valeurs manquantes spéciales (`.A`–`.Z`, `._`) ; labels ; formats/informats ; encodages gérés ; catalogues `.sas7bcat` ; index `.sas7bndx` ; fichiers chiffrés ; tailles supérieures à 2 Go ; streaming. Chaque case MUST porter une valeur (oui / non / partiel / inconnu) et une source (fichier source + ligne, ou document + section).

#### Scenario: Case sans source
- **WHEN** on parcourt toutes les cases de la matrice
- **THEN** aucune case n'est vide et chacune référence une source précise (chemin de fichier et ligne, ou document et section)

#### Scenario: Capacité non déterminable
- **WHEN** la source ne permet pas de conclure sur une capacité
- **THEN** la case porte la valeur `inconnu` accompagnée de la source consultée, plutôt qu'une valeur supposée

### Requirement: Points de désaccord identifiés
La section 3 MUST lister les champs ou comportements que les implémentations interprètent différemment, ignorent, ou écrivent avec une constante non expliquée (notamment dans le chemin d'écriture de ReadStat). Chaque point SHALL citer les implémentations concernées, leurs interprétations respectives et la source de chacune.

#### Scenario: Constante magique en écriture
- **WHEN** une implémentation écrit une valeur fixe non documentée dans un champ d'en-tête ou de sous-en-tête
- **THEN** le point de désaccord nomme le champ, la valeur écrite, l'emplacement dans le code source et l'absence d'explication

### Requirement: Manques rattachés aux changements de spécification
La section 4 MUST lister chaque manque (information absente, contradictoire ou non vérifiée dans les sources publiques) avec : une description, un impact sur le driver (`bloquant`, `dégradé` ou `confort`), le prompt de la série qui le traite parmi 02 à 05 (`specify-sas7bdat-read-format`, `specify-sas7bdat-compression`, `specify-sas-formats-encodings`, `specify-sas7bdat-writing`), et la méthode envisagée (analyse différentielle de fichiers ou décompilation).

#### Scenario: Manque non rattaché
- **WHEN** on vérifie la liste des manques
- **THEN** chaque manque est rattaché à exactement un prompt parmi 02, 03, 04 ou 05 et porte un impact et une méthode

#### Scenario: Préférence pour la méthode différentielle
- **WHEN** un manque peut être résolu en comparant des fichiers produits par SAS
- **THEN** la méthode indiquée est l'analyse différentielle, la décompilation n'étant retenue que lorsque l'analyse différentielle ne suffit pas, avec la justification

### Requirement: Contraintes de licence et recommandation
La section 5 MUST indiquer, pour chaque source inventoriée, la règle d'usage pendant la conception (lecture libre, lecture limitée à la documentation, lecture interdite du code source, etc.) et son effet sur la licence du futur driver, et SHALL recommander une licence pour le driver avec sa justification.

#### Scenario: Source sous licence copyleft
- **WHEN** une source est distribuée sous une licence copyleft (par exemple GPL)
- **THEN** la section 5 précise le risque pour un driver sous licence permissive et la règle de lecture associée

#### Scenario: Recommandation présente
- **WHEN** on lit la section 5
- **THEN** elle contient une licence recommandée pour le driver et une règle explicite pour chaque source de l'inventaire

### Requirement: Sources publiques traitées comme hypothèses
Le document MUST présenter les descriptions issues des sources publiques comme des hypothèses à vérifier, et SHALL associer à chaque affirmation sur le format un niveau de confiance (`confirmé`, `inféré`, `inconnu`) selon les conventions de `prompts/README.md` ; aucune affirmation de ce changement ne peut être `confirmé`, faute de preuve binaire ou runtime.

#### Scenario: Niveau de confiance
- **WHEN** le document décrit un champ ou un comportement du format d'après une source publique
- **THEN** l'affirmation est marquée `inféré` ou `inconnu`, jamais `confirmé`

### Requirement: Initialisation des fichiers de suivi
Le changement SHALL créer `rea_output/STATE.md` conforme au modèle de `prompts/README.md` (sections Dernière session, Avancement, Blocages ouverts, Décisions prises, Prochaine action), renseigné pour le prompt 00, et `rea_output/60_inconnues.md` ne contenant que son en-tête.

#### Scenario: STATE.md renseigné
- **WHEN** le changement est terminé
- **THEN** `rea_output/STATE.md` existe, indique le prompt 00 avec sa date et son résultat (terminé, partiel ou bloqué), l'état de chaque critère de fin, la licence recommandée parmi les décisions prises, et la prochaine action (`setup-sas-oracle-corpus`)

#### Scenario: Registre des inconnues vide
- **WHEN** on ouvre `rea_output/60_inconnues.md`
- **THEN** il contient un en-tête et aucune entrée

### Requirement: Périmètre strictement documentaire
Le changement MUST NOT accéder à `src/`, exécuter SAS, décompiler un binaire ni produire de conception ou de code Rust ; il SHALL s'appuyer uniquement sur des sources publiques.

#### Scenario: Aucune trace hors périmètre
- **WHEN** on examine les fichiers créés ou modifiés par le changement
- **THEN** seuls `rea_output/00_etat_de_l_art.md`, `rea_output/STATE.md` et `rea_output/60_inconnues.md` ont été créés, `src/` est inchangé et aucun document de conception Rust n'existe

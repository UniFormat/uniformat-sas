## Purpose

Définir, de façon vérifiée par SAS et par les lecteurs tiers, comment produire un fichier `.sas7bdat` valide : procédure d'écriture, classification de chaque champ, règles de construction, limites et cas non supportés, afin que la conception du driver Rust puisse implémenter l'écriture à partir de cette seule spécification.

## ADDED Requirements

### Requirement: Critère d'acceptation par les lecteurs
Un fichier `.sas7bdat` produit selon la spécification d'écriture SHALL être relu par SAS (`PROC CONTENTS`, `PROC PRINT`, `data _null_; set lib.t; run;`) avec un log sans `ERROR` ni `WARNING`, et par ReadStat (`pyreadstat`), pandas et Parso (si disponible), avec des données et métadonnées identiques au CSV attendu. L'identité octet à octet avec un fichier écrit par SAS MUST NOT être exigée.

#### Scenario: Fichier accepté par tous les lecteurs
- **WHEN** un fichier produit par l'écrivain de référence est relu par SAS, `pyreadstat`, pandas et Parso
- **THEN** chaque lecteur l'ouvre sans erreur, le log SAS ne contient ni `ERROR` ni `WARNING`, et les valeurs et métadonnées relues sont identiques au CSV attendu

#### Scenario: Écart détecté
- **WHEN** un lecteur rejette le fichier ou relit une valeur différente de l'attendu
- **THEN** l'écart est consigné dans la matrice de validation, la spécification est corrigée en premier, puis l'écrivain de référence

#### Scenario: Différences d'octets tolérées
- **WHEN** un fichier produit diffère d'un fichier SAS équivalent uniquement sur des champs classés `libre` (horodatages, champs libres)
- **THEN** le cas est considéré comme conforme

### Requirement: Écrivain de référence
Le changement SHALL fournir `rea_output/tools/ref_writer.py`, écrit à partir de la spécification de lecture (`rea_output/10_format_sas7bdat.md`) et de compression (`rea_output/15_compression.md`), capable de produire chaque cas de la matrice. Le cas initial MUST être une table d'une colonne numérique, une ligne, non compressée. Un champ sans règle connue MAY temporairement recopier la valeur observée dans un fichier SAS équivalent du corpus, à condition d'être marqué « recopié ».

#### Scenario: Cas minimal
- **WHEN** `ref_writer.py` produit une table d'une colonne numérique et d'une ligne, non compressée
- **THEN** le fichier passe la boucle de validation sur tous les lecteurs disponibles

#### Scenario: Champ recopié traçable
- **WHEN** l'écrivain recopie une valeur observée faute de règle connue
- **THEN** le champ est listé comme « recopié » avec le fichier du corpus et l'offset d'origine, en attente de classification

### Requirement: Classification de chaque champ
Chaque champ d'en-tête et de sous-en-tête écrit SHALL être classé `calculé` (avec formule), `constante` (avec valeur) ou `libre`, dans la table des champs de `rea_output/20_ecriture.md`, selon le modèle d'entrée de spécification et avec un niveau de confiance (`confirmé`, `inféré`, `inconnu`). La classification MUST reposer sur des mutations (0, valeur aléatoire, valeur hors limites) appliquées à un fichier valide et sur l'observation de la réaction de SAS (rejet, ignoré, changement de comportement).

#### Scenario: Mutation d'un champ recopié
- **WHEN** un champ recopié est muté à 0, à une valeur aléatoire puis à une valeur hors limites dans un fichier valide
- **THEN** la réaction de SAS à chaque mutation est enregistrée dans `rea_output/evidence/` et le champ reçoit la classe `calculé`, `constante` ou `libre`

#### Scenario: Rejet inexpliqué
- **WHEN** SAS rejette une mutation sans que les autres mutations permettent d'en déduire la règle
- **THEN** seuls les chemins de validation concernés de `sase7opn` / `sase7io` sont décompilés, et la preuve binaire est référencée dans l'entrée du champ

#### Scenario: Aucun champ recopié sans explication en fin de changement
- **WHEN** la table des champs est finalisée
- **THEN** aucun champ d'en-tête ou de sous-en-tête ne reste « recopié sans explication » ; tout champ non résolu figure dans `rea_output/60_inconnues.md`

### Requirement: Montée en complexité par cas validés
La matrice de validation SHALL couvrir, un cas validé à la fois : plusieurs colonnes num/char, longueurs numériques 3 à 8, missing spéciaux, labels, formats/informats, noms longs, noms et valeurs non ASCII, plusieurs pages (data et mix), beaucoup de colonnes (plusieurs sous-en-têtes de texte), 0 ligne, gros volume (> 2 Go si faisable), `COMPRESS=CHAR`, `COMPRESS=BINARY`, et un encodage autre qu'UTF-8. Les cas compressés MUST utiliser `rea_output/tools/ref_compress.py` issu de `specify-sas7bdat-compression`.

#### Scenario: Cas compressé
- **WHEN** l'écrivain produit une table en `COMPRESS=CHAR` puis en `COMPRESS=BINARY`
- **THEN** chaque fichier passe la boucle de validation sur tous les lecteurs disponibles

#### Scenario: Cas non faisable
- **WHEN** un cas ne peut être exécuté (par exemple le gros volume > 2 Go, ou un lecteur indisponible)
- **THEN** la matrice l'indique explicitement avec la raison, au lieu de le marquer OK

### Requirement: Matrice de validation
Le changement SHALL fournir `rea_output/tools/write_matrix.md` donnant, pour chaque cas × lecteur, le résultat `OK` ou `KO` et une référence au log correspondant, ainsi que la liste des lecteurs effectivement disponibles.

#### Scenario: Lecture de la matrice
- **WHEN** un relecteur consulte `write_matrix.md`
- **THEN** il trouve pour chaque cas de la montée en complexité et chaque lecteur un statut OK/KO et le chemin du log

### Requirement: Règles de construction
`rea_output/20_ecriture.md` SHALL décrire les règles de construction : ordre des sous-en-têtes, calcul de la taille de page et de la taille d'en-tête, répartition des lignes dans les pages (stratégie de remplissage), mise à jour des compteurs, et limites (noms 32 octets, labels 256 octets, char 32 767 octets, nombre maximal de colonnes), chacune avec un niveau de confiance et une preuve.

#### Scenario: Limite dépassée
- **WHEN** une limite documentée (par exemple un nom de 33 octets) est testée contre SAS
- **THEN** le comportement observé (rejet, troncature, avertissement) est documenté avec sa preuve

#### Scenario: Procédure pas à pas
- **WHEN** un lecteur de `20_ecriture.md` suit la procédure d'écriture
- **THEN** il peut construire un fichier valide sans consulter `rea_output/evidence/` ni une sortie de décompilation

### Requirement: Cas hors priorité documentés
L'ajout de lignes à un fichier existant, les lignes supprimées, les index, les contraintes d'intégrité et le chiffrement SHALL être listés dans `rea_output/20_ecriture.md` comme cas non supportés, avec seulement ce qui est observable, sans effort de reverse engineering dédié.

#### Scenario: Cas non supporté
- **WHEN** un utilisateur de la spec cherche comment écrire un fichier chiffré ou indexé
- **THEN** la spec indique que le cas n'est pas supporté et résume les seules observations disponibles

### Requirement: Traçabilité et état de session
Le changement MUST mettre à jour `rea_output/60_inconnues.md` (toute zone d'octets non expliquée), `rea_output/evidence/` (mutations, logs, décompilations) et `rea_output/STATE.md` (statut du prompt 05, critère de fin atteint ou non, blocages, prochaine action), et MUST NOT modifier `src/`.

#### Scenario: Fin de session
- **WHEN** la session du prompt 05 se termine
- **THEN** `rea_output/STATE.md` indique le statut (terminé, partiel ou bloqué) et si le critère de fin est atteint

### Requirement: Cadre de licence consigné
Avant toute décompilation ou mutation analysée, la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte SHALL être consignée dans `rea_output/STATE.md`. Le travail MUST NOT tenter de contourner la licence (`elssrv`, SETINIT).

#### Scenario: Confirmation absente
- **WHEN** la confirmation n'est pas encore consignée dans `rea_output/STATE.md`
- **THEN** la décompilation de `sase7opn` / `sase7io` n'est pas lancée et l'absence est inscrite dans les blocages ouverts

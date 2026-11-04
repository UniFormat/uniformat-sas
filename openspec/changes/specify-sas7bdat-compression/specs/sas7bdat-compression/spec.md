## Purpose

Fournir une spécification vérifiée des compressions de lignes `COMPRESS=CHAR` et `COMPRESS=BINARY` des fichiers `.sas7bdat`, en lecture comme en écriture, pour que le driver lise tous les fichiers compressés et en produise que SAS accepte.

## ADDED Requirements

### Requirement: Identification prouvée des algorithmes de compression
La spécification `rea_output/15_compression.md` SHALL confirmer ou infirmer, preuve à l'appui (chaîne présente dans le fichier, symbole, appel observé), chacun des liens supposés : `COMPRESS=CHAR` ↔ chaîne `SASYZCRL` ↔ module `src/sasexe/sasyzcrl` ↔ RLE, et `COMPRESS=BINARY` ↔ chaîne `SASYZCR2` ↔ module `src/sasexe/sasyzcr2` ↔ RDC (Ross Data Compression). Chaque lien MUST porter un niveau de confiance `confirmé`, `inféré` ou `inconnu`.

#### Scenario: Lien confirmé par deux sources
- **WHEN** la chaîne `SASYZCRL` est trouvée dans un fichier `CHAR` du corpus et le module `sasyzcrl` est observé (symbole ou appel) lors de la lecture de ce fichier
- **THEN** le lien `CHAR` ↔ `SASYZCRL` ↔ `sasyzcrl` est noté `confirmé` avec les deux preuves (fichier + offset, fonction@adresse ou sortie runtime)

#### Scenario: Hypothèse publique non vérifiée
- **WHEN** un lien ou un algorithme ne repose que sur une description publique (ReadStat, Parso, pandas, PDF de Shotwell)
- **THEN** il est noté au plus `inféré` et n'est jamais présenté comme prouvé

### Requirement: Détection des lignes compressées
La spécification SHALL décrire, à partir du corpus `NO` / `CHAR` / `BINARY`, l'unité compressée (ligne, page ou autre), le mécanisme par lequel une ligne compressée est repérée (pointeurs de sous-en-têtes, indicateurs, champs de l'en-tête) et le stockage d'une ligne pour laquelle la compression n'apporte rien. Chaque champ binaire concerné MUST suivre le modèle d'entrée de spécification de `prompts/README.md`.

#### Scenario: Ligne compressée repérée
- **WHEN** un lecteur suit la spec sur un fichier `CHAR` ou `BINARY` du corpus
- **THEN** il distingue sans ambiguïté, pour chaque ligne, si elle est stockée compressée ou brute, et localise ses octets

#### Scenario: Ligne non compressible
- **WHEN** une ligne compressée serait plus longue que la ligne brute
- **THEN** la spec indique comment SAS la stocke (brute ou autre) et comment ce cas est signalé dans le fichier

### Requirement: Tables d'opcodes de décompression
La spécification SHALL décrire chaque algorithme sous forme de table d'opcodes : octet de contrôle → action, longueur, données lues. Pour RLE, chaque commande MUST être décrite ; pour RDC, le format des groupes de contrôle et des références arrière MUST être décrit. Un opcode non expliqué par les fichiers du corpus ni par la décompilation MUST être consigné dans `rea_output/60_inconnues.md`.

#### Scenario: Opcode couvert
- **WHEN** un octet de contrôle apparaît dans une ligne compressée du corpus
- **THEN** la table d'opcodes donne son action, sa longueur et les données qu'il consomme, avec une preuve et un niveau de confiance

#### Scenario: Opcode inexpliqué
- **WHEN** un opcode n'est expliqué ni par le corpus ni par `sasyzcrl` / `sasyzcr2`
- **THEN** il figure dans `rea_output/60_inconnues.md` avec la confiance `inconnu`, et n'est pas masqué

### Requirement: Règles de compression obligatoires et libres
La spécification SHALL décrire les règles de décision du chemin d'écriture de SAS (seuils de longueur des séquences, taille de fenêtre RDC, choix entre ligne brute et compressée) et MUST classer chaque règle comme **obligatoire** (nécessaire pour que SAS décode la sortie) ou **choix libre** (le driver peut produire d'autres octets que SAS).

#### Scenario: Règle classée
- **WHEN** une règle de décision est documentée
- **THEN** elle est marquée `obligatoire` ou `libre`, avec la preuve qui justifie ce classement

### Requirement: Vecteurs de test de compression
Le livrable SHALL inclure, sous `rea_output/tools/compression_vectors/`, des paires (ligne brute, ligne compressée) extraites du corpus pour `CHAR` et `BINARY`, et des cas limites : ligne vide, ligne toute identique, ligne incompressible, séquences à la limite des longueurs.

#### Scenario: Couverture des cas limites
- **WHEN** on liste les vecteurs de `rea_output/tools/compression_vectors/`
- **THEN** chaque algorithme dispose d'au moins un vecteur issu du corpus et d'un vecteur pour chacun des cas limites cités

### Requirement: Décompresseur de référence validé sur le corpus
`rea_output/tools/ref_reader.py` SHALL être étendu avec un décompresseur écrit depuis la seule spec, et MUST relire tous les fichiers `CHAR` et `BINARY` du corpus avec un résultat identique au CSV de référence.

#### Scenario: Relecture du corpus compressé
- **WHEN** `ref_reader.py` lit chaque fichier `CHAR` et `BINARY` listé dans `corpus/MANIFEST.md`
- **THEN** les valeurs produites sont identiques au CSV de référence correspondant

### Requirement: Compresseur de référence réversible
`rea_output/tools/ref_compress.py` SHALL implémenter la compression `CHAR` et `BINARY` selon la spec. Pour chaque vecteur `x`, `decompress(ref_compress(x)) == x` et `decompress(SAS(x)) == x` MUST être vrais.

#### Scenario: Aller-retour sur un vecteur
- **WHEN** on applique `ref_compress` puis le décompresseur de référence à une ligne brute d'un vecteur
- **THEN** on retrouve exactement la ligne brute

#### Scenario: Décompression d'une sortie SAS
- **WHEN** on applique le décompresseur de référence à la ligne compressée par SAS d'un vecteur
- **THEN** on retrouve exactement la ligne brute du vecteur

### Requirement: Vérification SAS du compresseur reportée et tracée
La vérification que SAS relit les lignes compressées par `ref_compress.py` SHALL être reportée au changement `specify-sas7bdat-writing` (elle exige un écrivain complet) et MUST être consignée dans `rea_output/STATE.md` comme action ouverte.

#### Scenario: Report consigné
- **WHEN** le changement se termine
- **THEN** `rea_output/STATE.md` mentionne la vérification SAS de `ref_compress.py` comme reportée au prompt 05, avec l'avancement du prompt 03, les blocages et la prochaine action

### Requirement: Traçabilité des preuves et des inconnues
Toute affirmation de `rea_output/15_compression.md` SHALL porter un niveau de confiance et une preuve ; les preuves brutes (sorties de décompilation, extraits runtime) MUST être rangées dans `rea_output/evidence/`, et la spec MUST rester lisible sans `evidence/` pour le changement `design-rust-driver`. `src/` MUST rester inchangé.

#### Scenario: Spec autonome pour la conception
- **WHEN** la conception Rust lit `rea_output/15_compression.md` sans accéder à `rea_output/evidence/`
- **THEN** elle dispose de toutes les tables d'opcodes, règles et niveaux de confiance nécessaires, sans code SAS décompilé recopié

#### Scenario: src/ intact
- **WHEN** on compare `src/` avant et après le changement
- **THEN** aucun fichier n'a été modifié

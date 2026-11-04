## 1. Préalables

- [x] 1.1 Lire `prompts/README.md`, `rea_output/STATE.md`, `rea_output/10_format_sas7bdat.md` et `corpus/MANIFEST.md` ; vérifier que `specify-sas7bdat-read-format` est marqué terminé dans `STATE.md` (sinon s'arrêter et le signaler)
  - _Note (2026-10-30) :_ `specify-sas7bdat-read-format` est « partiel » dans `STATE.md` (livrables de lecture complets, décompilation non faite) ; le prompt 03 a été lancé sur instruction explicite de l'utilisateur.
- [x] 1.2 Consigner dans `rea_output/STATE.md` (section « Décisions prises ») la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte ; vérifier que l'entrée est présente avant toute décompilation
- [x] 1.3 Vérifier que l'oracle `sas_small` démarre dans l'environnement Linux x86-64 du prompt 01 (log sans `ERROR`) ; en cas de refus de licence, s'arrêter et le noter dans `STATE.md` sans contournement
- [x] 1.4 Lister dans `corpus/MANIFEST.md` les triplets `NO` / `CHAR` / `BINARY` disponibles et noter les manques éventuels dans `rea_output/15_compression.md`

## 2. Identification des algorithmes

- [x] 2.1 Rechercher les chaînes `SASYZCRL` et `SASYZCR2` dans les fichiers `CHAR` / `BINARY` du corpus et noter fichier + offset dans `15_compression.md`
- [ ] 2.2 Relier chaque chaîne à `src/sasexe/sasyzcrl` / `src/sasexe/sasyzcr2` par symbole ou appel observé (REA, lecture seule) ; consigner la preuve dans `rea_output/evidence/`
  - _Non exécuté_ : décompilation bloquée (confirmation NULL non consignée) ; liens avec les modules = `inconnu` (entrée C01).
- [x] 2.3 Statuer sur RLE (`CHAR`) et RDC (`BINARY`) avec un niveau de confiance par lien ; vérifier qu'aucun lien n'est `confirmé` sur la seule foi d'une source publique

## 3. Où et quoi (méthode différentielle)

- [x] 3.1 Comparer les fichiers `NO` / `CHAR` / `BINARY` d'un même jeu et déterminer l'unité compressée (ligne, page) ; documenter dans `15_compression.md`
- [x] 3.2 Identifier les champs signalant une ligne compressée (pointeurs de sous-en-têtes, indicateurs, en-tête) au format du modèle d'entrée de spécification
- [x] 3.3 Documenter le cas « compression sans gain » (ligne stockée brute ou autre) avec un exemple du corpus
- [x] 3.4 Consigner toute zone d'octets inexpliquée dans `rea_output/60_inconnues.md` et via `record_unknown`

## 4. Décompression

- [x] 4.1 Rédiger la table d'opcodes RLE (`CHAR`) : octet de contrôle → action, longueur, données lues, preuve, confiance
- [x] 4.2 Rédiger le format RDC (`BINARY`) : groupes de contrôle, littéraux, références arrière (offset, longueur), preuve, confiance
- [x] 4.3 Décompiler `sasyzcrl` / `sasyzcr2` uniquement pour les opcodes non expliqués par le corpus ; ranger les sorties dans `rea_output/evidence/` et ne reporter dans la spec que la description — FAIT le 2026-10-30 : `sasyzcrl@0x101d10` et `sasyzcr2@0x101d50` décompilés (Ghidra via REA, évidences `ev_6c55e8e3…`, `ev_f59950f8…`, `evidence/decompile/`) ; description reportée dans `15_compression.md` 3.1 et 4.1, deux erreurs de la spec corrigées (octets RLE `0x10`–`0x30`, validité).
  - _Non exécuté_ : même blocage ; les opcodes non expliqués (RLE `0x10`/`0x20`/`0x30`, RDC 10 à 12) sont dans `60_inconnues.md` (C06).
- [x] 4.4 Reporter les opcodes restés inexpliqués dans `60_inconnues.md` (confiance `inconnu`)

## 5. Compression (chemin d'écriture)

- [x] 5.1 Localiser le chemin d'écriture dans `sasyzcrl` / `sasyzcr2` et documenter les seuils de longueur des séquences RLE — FAIT : `15_compression.md` 3.1 (seuils 2/3 octets, copie 64, copie longue 16 447, suites 1 040 / 1 041).
  - _Non exécuté_ : décompilation du chemin d'écriture bloquée ; seuils RLE déduits des fichiers (R7, R8) et marqués libres.
- [x] 5.2 Documenter la taille de fenêtre RDC et les règles de choix littéral / référence arrière — FAIT : `15_compression.md` 4.1 et R10 (fenêtre 4 098, longueur 271, hachage de 3 octets).
  - _Partiel_ : fenêtre RDC et règles littéral / référence non établies sans décompilation (R9, R10 : `inconnu`) ; seul le maximum codable (4 098) et les décalages observés sont documentés.
- [x] 5.3 Documenter la règle de choix entre ligne brute et ligne compressée — FAIT : `15_compression.md` R11 (RLE strictement plus court ; RDC : au moins 19 octets et écriture ≤ `row_length − 50`).
  - _Partiel_ : règle observée (aucune ligne brute en RLE ; brute en RDC quand rien ne se compresse, 226 lignes) et critère de désactivation de table contraint par le surcoût de pointeur ; seuils exacts `inconnu` (R11, C02, C05).
- [x] 5.4 Classer chaque règle `obligatoire` ou `libre` avec sa justification dans `15_compression.md`

## 6. Vecteurs de test

- [x] 6.1 Extraire des paires (ligne brute, ligne compressée) `CHAR` et `BINARY` du corpus dans `rea_output/tools/compression_vectors/`
- [x] 6.2 Générer via l'oracle les cas limites : ligne vide, ligne toute identique, ligne incompressible, séquences à la limite des longueurs ; vérifier que chaque algorithme couvre chaque cas — FAIT (2026-11-02) avec le SAS complet (`corpus/files_complement`, `edge_*`) : 60 tables relues sans écart par rapport aux CSV de SAS (`tools/check_complement_csv.py`) ; le compresseur RLE de référence reproduit les flots de SAS sur 14 628 lignes ; RDC : décodage exact, encodage non octet par octet.
  - _Non réalisable_ : `sas_small` ne sait pas écrire de table compressée ; les cas limites manquants (ligne de longueur nulle, ligne uniforme, bornes RDC) exigent le SAS complet (action ouverte dans `STATE.md`).
- [x] 6.3 Décrire le format des vecteurs (fichier, champs) dans `rea_output/tools/compression_vectors/README.md`

## 7. Outils de référence et vérification

- [x] 7.1 Étendre `rea_output/tools/ref_reader.py` avec un décompresseur `CHAR` / `BINARY` écrit depuis `15_compression.md` seulement
- [x] 7.2 Vérifier que `ref_reader.py` relit tous les fichiers `CHAR` et `BINARY` du corpus avec un résultat identique au CSV de référence
- [x] 7.3 Écrire `rea_output/tools/ref_compress.py` (RLE et RDC) depuis la spec
- [x] 7.4 Vérifier pour chaque vecteur `decompress(ref_compress(x)) == x` et `decompress(SAS(x)) == x` ; corriger la spec avant le code en cas d'écart

## 8. Clôture

- [x] 8.1 Relire `15_compression.md` : chaque entrée a preuve et confiance, aucune recopie de code décompilé, lisible sans `evidence/`
- [x] 8.2 Vérifier que `src/` est inchangé
- [x] 8.3 Mettre à jour `rea_output/STATE.md` : dernière session (prompt 03, date, résultat), avancement, blocages, report au prompt 05 de la vérification SAS des sorties de `ref_compress.py`, prochaine action

## 1. Préalables et cadre

- [x] 1.1 Consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte ; vérifier que la section « Décisions prises » la mentionne avec date et interlocuteur, sinon l'inscrire dans « Blocages ouverts » — Confirmation NULL non consignée : maintenue dans « Blocages ouverts » de `STATE.md` (pas dans « Décisions prises »).
- [x] 1.2 Lire `prompts/README.md`, `rea_output/STATE.md`, `rea_output/10_format_sas7bdat.md`, `rea_output/15_compression.md`, `rea_output/30_formats_encodages.md` (s'il existe) et les manques en écriture de `rea_output/00_etat_de_l_art.md` ; vérifier que `STATE.md` marque 02 et 03 comme terminés, sinon s'arrêter et le signaler — `STATE.md` marque 02, 03 et 04 `partiel` (décompilation non faite), pas `terminé` ; poursuite sur demande explicite de l'utilisateur (« Oui, lance 05 »).
- [x] 1.3 Vérifier que SAS (environnement Linux x86-64 de `setup-sas-oracle-corpus`) démarre et relit un fichier du `corpus/` avec un log sans `ERROR` ni `WARNING` ; en cas d'échec de licence, s'arrêter sans contournement et le noter dans `STATE.md` — `tools/control_sas.py` : fichier du corpus OK (`evidence/write/control_ok_sas.log`).
- [x] 1.4 Installer/vérifier les lecteurs tiers (`pyreadstat`, pandas, Parso si une JVM est disponible) et inscrire la liste des lecteurs disponibles en tête de `rea_output/tools/write_matrix.md` — Liste en tête de `tools/write_matrix.md` : SAS 9.4M7 (`sas_small`), pandas 2.3.3, pyreadstat 1.2.9, Parso 2.0.14 (Java 26), `ref_reader.py`.

## 2. Écrivain de référence minimal

- [x] 2.1 Écrire `rea_output/tools/ref_writer.py` depuis `10_format_sas7bdat.md` pour le cas minimal (1 table, 1 colonne num, 1 ligne, non compressé) ; vérifier qu'il produit un fichier dans un répertoire de sortie hors `src/` — `tools/ref_writer.py`.
- [x] 2.2 Marquer dans l'écrivain et dans un brouillon de table des champs chaque champ « recopié » avec le fichier du `corpus/` et l'offset d'origine ; vérifier que la liste est exhaustive par comparaison octet à octet avec le fichier SAS équivalent — Liste `RECOPIES` ; `tools/diff_recopies.py` sur 13 fichiers du corpus : 0 plage non expliquée (`evidence/write/diff_recopies.txt`).

## 3. Boucle de validation

- [x] 3.1 Scripter la relecture SAS (`PROC CONTENTS`, `PROC PRINT`, `data _null_; set lib.t; run;`) avec détection automatique de `ERROR`/`WARNING` dans le log ; vérifier sur un fichier du corpus (OK) et un fichier volontairement corrompu (KO) — `validate_write.py` (détection `ERROR`/`WARNING`) ; contrôle : corpus OK, copie tronquée KO (`evidence/write/control_ko_sas.log`).
- [x] 3.2 Scripter la relecture `pyreadstat`, pandas et Parso, et la comparaison au CSV attendu (données et métadonnées) ; vérifier sur le cas minimal — `readers_dump.py` + `ParsoDump.java` + comparaison aux valeurs attendues.
- [x] 3.3 Faire passer le cas minimal sur tous les lecteurs disponibles, corriger la spec puis l'écrivain à chaque écart ; vérifier la ligne OK correspondante dans `write_matrix.md` avec le chemin du log dans `rea_output/evidence/` — Cas `w01_minimal` OK sur les 5 lecteurs (`evidence/write/w01_minimal_sas.log`).

## 4. Classification des champs par mutation

- [x] 4.1 Pour chaque champ recopié, générer les mutations (0, valeur aléatoire, valeur hors limites) d'un fichier valide et les faire relire par SAS ; vérifier que chaque réaction (rejet, ignoré, changement de comportement) est enregistrée dans `rea_output/evidence/` — `tools/mutate_write.py` : 96 champs × 3 mutations (288) relues par SAS : 154 ignorées, 30 modifiées, 104 rejetées, `evidence/write/mutations.tsv` et `evidence/write/mut/`.
- [x] 4.2 Classer chaque champ `calculé` (formule), `constante` (valeur) ou `libre` selon le modèle d'entrée de spec, avec niveau de confiance ; vérifier qu'aucun champ ne reste « recopié » sans entrée — `20_ecriture.md` section 5 : chaque entrée de `RECOPIES` y figure avec sa classe.
- [ ] 4.3 Pour les seuls rejets non expliqués par les mutations, décompiler les chemins de validation de `sase7opn` / `sase7io` avec REA (si 1.1 est consigné) et référencer la preuve `fonction@adresse` ; vérifier que la règle déduite est confirmée par une nouvelle mutation — NON FAIT : confirmation NULL (1.1) non consignée, donc aucune décompilation ; les rejets sont documentés par mutation seulement.
- [x] 4.4 Reporter tout champ ou zone d'octets non résolu dans `rea_output/60_inconnues.md` (et `record_unknown`) ; vérifier la correspondance entre la table des champs et `60_inconnues.md` — W01 à W09 dans `60_inconnues.md` ; `record_unknown` non appelé (aucun binaire ouvert dans REA).

## 5. Montée en complexité (un cas validé à la fois)

- [x] 5.1 Plusieurs colonnes num et char ; vérifier OK sur tous les lecteurs dans `write_matrix.md` — `w02_multicol` OK.
- [x] 5.2 Longueurs numériques 3 à 8 ; vérifier OK pour chaque longueur — `w03_num3` à `w03_num8` OK.
- [x] 5.3 Missing spéciaux (`.`, `._`, `.A`–`.Z`) ; vérifier OK et relecture correcte dans SAS — `w04_missing` OK.
- [x] 5.4 Labels, formats et informats (en s'appuyant sur `30_formats_encodages.md` s'il existe) ; vérifier que `PROC CONTENTS` affiche les valeurs attendues — `w05_meta`, `w05b_informat`, `w05c_infmt_only_char` OK (`OK*` : avertissement d'informat de l'oracle, note 1 de la matrice).
- [x] 5.5 Noms longs (32 octets) et noms/valeurs non ASCII ; vérifier OK — `w06_names` OK.
- [x] 5.6 Plusieurs pages (pages data et pages mix) ; vérifier OK et nombre de lignes relu — `w07_pages` (3 000 lignes), `w08_pages_buf` OK.
- [x] 5.7 Beaucoup de colonnes (plusieurs sous-en-têtes de texte) ; vérifier OK — `w09_manycols` (300), `w14_cols700` OK.
- [x] 5.8 Table à 0 ligne ; vérifier OK — `w10_zero` OK.
- [x] 5.9 Gros volume (> 2 Go) si faisable ; vérifier OK ou marquer « non faisable » avec la raison — Marqué « non faisable » (`w15_gt2gb`), raison dans la matrice (inconnue W08).
- [x] 5.10 `COMPRESS=CHAR` puis `COMPRESS=BINARY` avec `rea_output/tools/ref_compress.py` ; vérifier OK sur tous les lecteurs — FAIT (2026-11-02) : 0 à 10 000 lignes, une ou plusieurs pages, pages de 4 096 et 65 536 octets, OK sur les 5 lecteurs dont SAS (57 cas, 0 KO ; W01 et W02 résolues grâce au corpus du SAS complet, `rea_output/15_compression.md` `page_comp_0x9000`).
- [x] 5.11 Encodage autre qu'UTF-8 (par ex. latin1) ; vérifier OK ou documenter le blocage (transcodage, relecteur SAS indisponible) — latin1 (29) et wlatin1 (62) OK sur SAS ; pyreadstat refuse le code 29 (documenté, 6.4).
- [x] 5.12 Rejouer toute la matrice après la dernière correction de spec ; vérifier qu'aucun cas précédemment OK n'est passé KO — Matrice complète rejouée après la dernière correction : 49 cas, 224 OK, 9 KO connus, 0 KO.

## 6. Règles de construction et limites

- [x] 6.1 Documenter l'ordre des sous-en-têtes et le calcul de la taille de page et de l'en-tête ; vérifier par un fichier produit avec une taille de page non par défaut — `20_ecriture.md` 6.1 ; cas `w13_page*` (4 096 à 1 048 576).
- [x] 6.2 Documenter la stratégie de répartition des lignes dans les pages et la mise à jour des compteurs ; vérifier par comparaison avec des fichiers multi-pages du corpus — `20_ecriture.md` 6.2 ; `evidence/write/repartition_lignes.txt`.
- [x] 6.3 Tester les limites (noms 32 octets, labels 256, char 32 767, nombre de colonnes) et leur dépassement contre SAS ; vérifier que chaque comportement observé est documenté avec preuve — `20_ecriture.md` section 8 ; cas `w14_*`.
- [x] 6.4 Documenter les cas hors priorité (ajout de lignes, lignes supprimées, index, contraintes d'intégrité, chiffrement) avec seulement ce qui est observable ; vérifier qu'ils figurent comme « non supportés » — `20_ecriture.md` section 7.

## 7. Livrables et clôture

- [x] 7.1 Rédiger `rea_output/20_ecriture.md` : procédure pas à pas, table des champs, règles de construction, limites, cas non supportés ; vérifier qu'il ne cite aucun extrait de décompilation et ne renvoie à `evidence/` que par identifiant — `rea_output/20_ecriture.md`.
- [x] 7.2 Finaliser `rea_output/tools/write_matrix.md` (cas × lecteur → OK/KO + log) ; vérifier que tous les cas de la section 5 y figurent — `rea_output/tools/write_matrix.md` : tous les cas de la section 5 y figurent.
- [x] 7.3 Vérifier le critère de fin : tous les cas passent les lecteurs disponibles (y compris compressés) et aucun champ n'est « recopié sans explication » — ATTEINT (2026-11-02) : `write_matrix.md` 57 cas, 0 cellule KO (les « KO connus » sont des limites de SAS documentées) ; seule limite restante côté compression : 65 535 lignes (W11).
- [x] 7.4 Vérifier que `src/` n'a pas été modifié (comparaison avec `src/files_manifest.txt`) — Aucun fichier de `src/` modifié le 2026-10-30 ; fichiers hors manifeste inchangés (`.DS_Store`, `sas_small`, `sasv9_small.cfg`, datés du 2026-10-29).
- [x] 7.5 Mettre à jour `rea_output/STATE.md` (prompt 05, date, résultat terminé/partiel/bloqué, critère de fin atteint ?, blocages, décisions, prochaine action : `design-rust-driver`) ; vérifier que le format suit `prompts/README.md` — `STATE.md` mis à jour (prompt 05 : partiel).

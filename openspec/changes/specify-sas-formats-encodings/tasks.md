## 1. Préparation et cadre licence

- [x] 1.1 Consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte ; vérifier que la ligne « Décisions prises » la mentionne avec date et interlocuteur
  - _Résultat (2026-10-30)_ : la confirmation n'est **pas** obtenue ; le point reste dans « Blocages ouverts » de `STATE.md` et toute décompilation (sections 2, 4.7, 5.5, 6.1) est exclue.
- [x] 1.2 Lire `prompts/README.md`, `rea_output/STATE.md`, `rea_output/10_format_sas7bdat.md` et `corpus/MANIFEST.md` ; noter dans `STATE.md` la session 04 démarrée et si `PROC FORMAT` fonctionne dans l'oracle
  - _Note_ : `PROC FORMAT` ne fonctionne pas dans `sas_small` (`ERROR: PROCEDURE FORMAT not found`) ; le prompt 03 était `partiel` à l'ouverture de cette session.
- [x] 1.3 Relever dans `rea_output/00_etat_de_l_art.md` ce qui est déjà publié sur formats, catalogues et encodages ; lister les manques ciblés en tête d'un brouillon de `rea_output/30_formats_encodages.md`
- [x] 1.4 Créer `rea_output/tools/` et la section 04 de `rea_output/evidence/` ; vérifier leur présence

## 2. Partie A — Inventaire des formats et informats natifs

- [x] 2.1 Extraire avec REA les tables de noms, chaînes et symboles de `src/sasexe/tkefmt.so` ; enregistrer la sortie brute dans `rea_output/evidence/` — FAIT le 2026-10-31 : `tkefmt.so` n'a pas de table de noms (bundle `tkefmt_bundle.json`) ; `30` 6.4.
  - _Non exécuté_ : décompilation bloquée. Remplacé par une liste de 241 noms de la documentation publique testés dans l'oracle (`evidence/fmt_existence.log`).
- [x] 2.2 Faire de même pour chaque module `tkefmf*.so` et `tkefmi*.so` ; vérifier que chaque module listé dans `src/files_manifest.txt` est couvert — FAIT : 9 modules `tkefmf*`/`tkefmi*` avec table de noms et `tkefmprf.so` sans table ; `evidence/decompile/formats_natifs.tsv` (574 lignes).
  - _Non exécuté_ : même blocage ; le module d'origine de chaque format reste `inconnu` (entrée F01).
- [x] 2.3 Consolider l'inventaire (nom, format/informat, module, preuve, confiance) dans `30_formats_encodages.md` ; recouper avec la documentation publique et marquer les écarts — FAIT le 2026-10-31 : `30` 6.4 (comparaison) et 1.1 (388 couples natifs ajoutés avec famille, type Arrow, module) ; les 55 couples absents des binaires sont résumés, liste complète à extraire par comparaison des deux TSV.
  - _Partiel_ : inventaire consolidé (nom, format ou informat, présence dans l'oracle, confiance) mais sans module ni preuve binaire.
- [x] 2.4 Lister les formats NLS (`NLDATE`…) et localisés dans une section hors priorité, sans spécification ; vérifier qu'aucun algorithme n'y est décrit

## 3. Partie A — Classification format → type Arrow

- [x] 3.1 Classer chaque format inventorié (date jours depuis 1960-01-01, datetime secondes depuis 1960-01-01T00:00:00, heure secondes, numérique, décimal à échelle fixe, caractère) avec son type Arrow cible
- [x] 3.2 Identifier et documenter les formats ambigus avec la règle de repli proposée au driver
- [x] 3.3 Placer la table de classification en tête de `30_formats_encodages.md` ; vérifier par script que tout format de l'inventaire y figure (zéro manquant)

## 4. Partie A — Algorithmes d'affichage

- [x] 4.1 Générer dans l'oracle un jeu de valeurs limites (arrondis, grandes/petites valeurs, largeurs insuffisantes, missing `.`, `._`, `.A`–`.Z`, chaînes vides) pour les formats de la liste A.3 ; enregistrer les sorties `PROC PRINT` dans `rea_output/evidence/` et les fichiers dans `corpus/` (MANIFEST mis à jour)
- [ ] 4.2 Spécifier `w.d`, `BEST`, `E`, `Z` (arrondi, bascule en notation E, `*`) ; vérifier contre les sorties de 4.1
  - _Partiel_ : `w.d`, `BEST`, `E`, `Z` reproduits en grande partie ; le repli en notation E et les petites valeurs divergent (F02, F03).
- [ ] 4.3 Spécifier `COMMA`, `DOLLAR`, `PERCENT` ; vérifier contre les sorties de 4.1
  - _Partiel_ : `COMMA`, `DOLLAR`, `PERCENT`, `NEGPAREN` reproduits hors repli en notation E (F02).
- [ ] 4.4 Spécifier `DATE`, `DATETIME`, `TIME`, `YYMMDD`, `DDMMYY`, `MMDDYY`, `E8601DA`, `E8601DT` et variantes ISO 8601 ; vérifier contre les sorties de 4.1
  - _Partiel_ : formats de date, datetime et heure reproduits à plus de 90 % ; ISO 8601 absents de l'oracle (F04) ; dates hors plage et grandes heures (F05, F06).
- [ ] 4.5 Spécifier `$CHAR` et `$` (espaces de tête/fin, troncature) ; vérifier contre les sorties de 4.1
  - _Partiel_ : `$CHAR`, `$`, `$QUOTE` reproduits sans écart ; `$HEX`, `$BINARY`, `$OCTAL` ne s'affichent pas comme leur nom dans l'oracle (F04).
- [x] 4.6 Écrire le vérificateur d'affichage dans `rea_output/tools/` et vérifier qu'il reproduit `PROC PRINT` sur tout le corpus (zéro écart, ou écarts consignés dans `60_inconnues.md`)
- [ ] 4.7 Décompiler uniquement les routines dont le comportement reste ambigu après 4.2–4.6 ; archiver les preuves dans `evidence/` et relever la confiance à `confirmé` quand binaire + runtime concordent
  - _Non exécuté_ : décompilation bloquée.

## 5. Partie B — Catalogues `.sas7bcat`

- [x] 5.1 Si `PROC FORMAT` fonctionne : générer des catalogues avec `VALUE` (valeurs, plages, `OTHER`), `$VALUE`, `INVALUE`, `PICTURE`, plusieurs formats par catalogue, et les `CNTLOUT=` de référence ; sinon recenser les catalogues de `corpus/external/` ; sinon marquer la partie B bloquée avec la raison dans `30_formats_encodages.md` et `STATE.md`
- [ ] 5.2 Produire des paires de catalogues ne différant que d'un élément et comparer les octets ; enregistrer les diffs dans `evidence/`
  - _Non réalisable ici_ : `PROC FORMAT` absent de `sas_small` ; exige le SAS complet (un seul catalogue disponible).
- [x] 5.3 Utiliser `src/sashelp/core.sas7bcat` uniquement pour la structure générique (en-tête, répertoire) ; vérifier qu'aucun exemple de format utilisateur n'en est tiré
- [ ] 5.4 Spécifier en-tête, répertoire des entrées, entrées `FORMAT`/`FORMATC`/`INFMT`/`INFMTC`, plages et libellés selon le modèle d'entrée de `prompts/README.md`
  - _Partiel_ : en-tête et répertoire décrits, entrées non décodées (F07 à F09).
- [ ] 5.5 Décompiler les seules zones restées inconnues après 5.2 ; consigner les octets non expliqués dans `60_inconnues.md`
  - _Non exécuté_ : décompilation bloquée ; octets non expliqués dans F07 à F09.
- [ ] 5.6 Écrire le lecteur de référence de catalogue dans `rea_output/tools/` et vérifier qu'il restitue au moins un catalogue à l'identique de `CNTLOUT=`
  - _Partiel_ : `catalog_dump.py` lit répertoire et libellés ; le critère « identique à `CNTLOUT=` » n'est pas atteint.

## 6. Partie C — Encodages

- [x] 6.1 Extraire la table des encodages de `tknlsenc.so`, `tknlsicv.so` et `src/nls/` ; enregistrer les sorties dans `evidence/` — FAIT pour `tknlsenc.so` et `tknlsicv.so` (`30` 6.3 et 6.5, `encodages_natifs.tsv`) ; `src/nls/` non étudié.
  - _Non exécuté_ : décompilation bloquée ; table construite à partir de ReadStat, pandas et Parso (hypothèses) et des fichiers du corpus.
- [x] 6.2 Relever les codes d'encodage présents dans les en-têtes de tous les fichiers de `corpus/` et ceux cités par les sources publiques
- [x] 6.3 Construire la table code en-tête → nom SAS → nom WHATWG/IANA (`encoding_rs`) avec confiance par ligne ; vérifier par script que tout code de 6.2 y figure
- [x] 6.4 Observer dans l'oracle le comportement sur octets invalides et sur caractère multi-octets tronqué en fin de champ ; documenter avec preuve runtime
- [x] 6.5 Écrire le vérificateur d'encodage dans `rea_output/tools/` et vérifier que le texte décodé égale la sortie de l'oracle pour les fichiers du corpus

## 7. Clôture

- [x] 7.1 Relire `30_formats_encodages.md` : chaque affirmation a une confiance et une preuve ; aucune référence à `evidence/` n'est nécessaire pour comprendre la spec (séparation pour `design-rust-driver`)
- [x] 7.2 Vérifier les quatre critères de fin du prompt 04 (classification complète, affichage = `PROC PRINT`, catalogue = `CNTLOUT=` ou bloqué motivé, encodages couverts) et noter le résultat
- [x] 7.3 Mettre à jour `rea_output/60_inconnues.md` avec tous les inconnus restants de la session
- [x] 7.4 Mettre à jour `rea_output/STATE.md` (dernière session 04 et résultat, avancement, blocages, décisions, prochaine action : 05 `specify-sas7bdat-writing` ou 06 `design-rust-driver`)

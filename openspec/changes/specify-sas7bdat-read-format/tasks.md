## 1. Préalables et licence

- [x] 1.1 Consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte (date, source, périmètre) ; vérifier que la mention figure sous « Décisions prises », sinon l'inscrire dans « Blocages ouverts » et ne pas exécuter la section 4
- [x] 1.2 Lire `prompts/README.md`, `rea_output/STATE.md`, `rea_output/00_etat_de_l_art.md` et `corpus/MANIFEST.md` ; vérifier que les livrables de `survey-sas7bdat-prior-art` et `setup-sas-oracle-corpus` sont présents, sinon arrêter et le noter dans `STATE.md`
- [x] 1.3 Créer `rea_output/tools/`, `rea_output/evidence/` et, s'il n'existe pas, `rea_output/60_inconnues.md` ; vérifier leur présence par `ls rea_output`
- [x] 1.4 Dresser depuis `corpus/MANIFEST.md` la liste des fichiers non compressés (générés et `external/`) et des paires différentielles (un seul paramètre varie) ; la consigner en tête de `rea_output/10_format_sas7bdat.md` et vérifier que chaque fichier listé existe

## 2. Hypothèse de départ (état de l'art)

- [x] 2.1 Créer le squelette de `rea_output/10_format_sas7bdat.md` par structure (en-tête, pages, sous-en-têtes, colonnes, valeurs, lignes supprimées, contrôles à l'ouverture) ; vérifier que chaque section existe
- [x] 2.2 Reporter champ par champ la structure de `00_etat_de_l_art.md` selon le modèle d'entrée (emplacement 32/64 bits, taille/type, sémantique, valeurs observées, écriture, preuve, confiance `inféré`) ; vérifier qu'aucune entrée n'est `confirmé` à ce stade
- [x] 2.3 Reporter les manques connus de l'état de l'art dans `rea_output/60_inconnues.md` ; vérifier le renvoi croisé depuis la spec

## 3. Analyse différentielle du corpus

- [x] 3.1 Écrire dans `rea_output/tools/` un script d'hexdump annoté par champ (à partir des offsets de la spec) ; vérifier qu'il s'exécute sur un fichier du corpus et produit un hexdump dans `rea_output/evidence/`
- [x] 3.2 Écrire dans `rea_output/tools/` un script de diff binaire de paires listant les offsets qui varient ; vérifier sur une paire connue que seuls les offsets attendus diffèrent
- [x] 3.3 Confirmer ou corriger les champs de l'en-tête (magic, alignements, endianness, plateforme, code d'encodage, horodatages epoch 1960, nom/type de table, versions SAS/OS, tailles d'en-tête/page, nombre de pages, réservés) ; vérifier que chaque entrée cite paire + offset en preuve
- [x] 3.4 Confirmer ou corriger l'en-tête de page, les types de pages (meta, data, mix, amd, autres), les pointeurs de sous-en-têtes, la position des lignes et le padding/alignement ; vérifier sur une page data et une page mix
- [x] 3.5 Confirmer ou corriger les signatures (32/64 bits, LE/BE) et le contenu des sous-en-têtes row size, column size, subheader counts, column text, column names, column attributes, format and label, column list ; consigner tout sous-en-tête inconnu dans `60_inconnues.md`
- [x] 3.6 Confirmer ou corriger les métadonnées de colonnes (type, longueur, offset, nom, label, format/informat via références au texte) ; vérifier contre les sorties `PROC CONTENTS` de référence du corpus
- [x] 3.7 Confirmer ou corriger le codage des valeurs (numériques tronqués 3–8 octets, missing standard et spéciaux, caractères et padding) et des lignes supprimées ; vérifier sur les fichiers du corpus dédiés
- [x] 3.8 Lister les champs restés `inféré` ou `inconnu` après différentiel dans une section « Cibles de décompilation » de la spec ; vérifier que la liste est non ambiguë (champ + structure + offset)

## 4. Décompilation ciblée (REA)

> **Non exécutée (2026-10-30)** : la confirmation interne NULL de l'analyse d'interopérabilité n'est pas consignée dans `rea_output/STATE.md` (tâche 1.1 : point inscrit dans « Blocages ouverts »). Aucun binaire de `src/` n'a été ouvert. Les cibles sont listées dans la section 8 de `rea_output/10_format_sas7bdat.md`.

- [ ] 4.1 Vérifier que la tâche 1.1 est faite ; ouvrir avec REA les modules `sase7io`, `sase7opn`, `sase7l`, `sase7ld`, `sase7lu`, `sase7xgt`, `sase7xop`, `sase7xrt`, `src/dbcs/sasexe/sase7`, `sase7yse` en lecture seule ; vérifier que `src/` n'est pas modifié
- [ ] 4.2 Repérer les points d'entrée par `search_strings` / `search_procedures` (ouverture de fichier, lecture de page, décodage de ligne) ; noter fonction@adresse dans `rea_output/evidence/`
- [ ] 4.3 Tracer avec `analyze_function` / `trace_call_path` le chemin ouverture → décodage d'une ligne pour chaque cible de 3.8 ; mettre à jour l'entrée (preuve fonction@adresse, comportement en prose, sans pseudo-code) et passer en `confirmé` si double preuve
- [ ] 4.4 Identifier les contrôles effectués à l'ouverture (champ, condition de rejet, erreur) et les relier aux messages de `src/sasmsg/` ; vérifier que chaque contrôle documenté cite son message
- [ ] 4.5 Enregistrer via `record_unknown` et dans `60_inconnues.md` toute zone restée inexpliquée ; vérifier la concordance entre les deux
- [ ] 4.6 Exporter le bundle REA avec `export_evidence_bundle` dans `rea_output/evidence/` ; vérifier sa présence

## 5. Lecteur de référence

- [x] 5.1 Écrire `rea_output/tools/ref_reader.py` en Python standard, à partir de la seule spec (sans ReadStat, pandas, Parso, ni consultation de `evidence/`) ; vérifier l'absence de ces imports
  - _Réalisé en deux temps (2026-10-30) :_ `ref_reader.py` a été écrit pendant l'analyse, donc **pas** à partir de la seule spec ; l'exigence d'indépendance est tenue par `ref_reader_spec_only.py`, écrit par un agent n'ayant lu que `10_format_sas7bdat.md` (imports : `struct`, `math`). Les 18 lacunes qu'il a signalées sont dans `evidence/lecteur_independant_rapport_lacunes.md`.
- [x] 5.2 Écrire dans `rea_output/tools/` un script de comparaison de la sortie de `ref_reader.py` au CSV de référence de chaque fichier ; vérifier qu'il signale un écart volontairement introduit
- [x] 5.3 Lancer la lecture de tous les fichiers non compressés du corpus (générés et `external/`, au moins LE/64 bits) ; pour chaque écart, corriger d'abord `10_format_sas7bdat.md` puis le script, jusqu'à zéro écart
- [x] 5.4 Vérifier les lignes supprimées et les missing spéciaux sur les fichiers dédiés : sortie identique au CSV de référence

## 6. Complétude et clôture

- [x] 6.1 Contrôler, pour l'en-tête et chaque sous-en-tête (32 et 64 bits), que chaque octet est décrit dans la spec ou listé dans `60_inconnues.md` ; consigner le résultat du contrôle dans la spec
- [x] 6.2 Relire la spec : chaque entrée suit le modèle, chaque `confirmé` a une double preuve, aucun pseudo-code SAS, compression/formats/écriture seulement par renvoi à `specify-sas7bdat-compression`, `specify-sas-formats-encodings`, `specify-sas7bdat-writing`
- [x] 6.3 Mettre à jour `rea_output/STATE.md` (dernière session : prompt 02, date, résultat ; avancement ; critère de fin atteint ou non ; blocages ; décisions ; prochaine action : `specify-sas7bdat-compression` et `specify-sas-formats-encodings`) ; vérifier le format défini dans `prompts/README.md`

## Context

Voir `proposal.md` (Why) pour la motivation. Points de départ :

- `specify-sas7bdat-read-format` (02) a localisé dans `rea_output/10_format_sas7bdat.md` les noms de format (+ largeur, décimales) des colonnes et le code d'encodage de l'en-tête ; ce changement en donne la **sémantique**.
- `setup-sas-oracle-corpus` (01) fournit l'oracle `sas_small` (SAS 9.4 TS1M7, ELF x86-64 Linux, UTF-8, `src/` en lecture seule) et `corpus/MANIFEST.md`. `sas_small` est validé pour `PROC CONTENTS`, `PROC PRINT` et l'étape DATA ; `PROC FORMAT` n'est pas garanti (à vérifier dans `rea_output/STATE.md`).
- `sas_small` ne contient que les modules NLS UTF-8 : les fichiers d'autres encodages ne peuvent être lus par l'oracle sans modules supplémentaires.
- L'hôte est macOS ; SAS tourne dans un Linux x86-64 (conteneur, VM ou machine distante).
- `survey-sas7bdat-prior-art` (00) délimite ce qui est déjà publié (ReadStat, Parso, pandas, Shotwell) : la décompilation ne vise que les manques.
- Peut tourner en parallèle de `specify-sas7bdat-compression` (03) ; nourrit `specify-sas7bdat-writing` (05) et `design-rust-driver` (06).

## Goals / Non-Goals

**Goals:**
- Une table de classification format → type Arrow directement consommable par 06, en tête de `rea_output/30_formats_encodages.md`.
- Des vérificateurs exécutables (`rea_output/tools/`) qui rendent les critères de fin mesurables : affichage vs `PROC PRINT`, catalogue vs `CNTLOUT=`, décodage vs oracle.
- Une table d'encodages exploitable avec `encoding_rs`.

**Non-Goals:**
- Spécifier les formats NLS/localisés (listés seulement).
- Écrire des catalogues `.sas7bcat` (l'écriture `.sas7bdat` relève de 05).
- Écrire le moindre code du driver Rust, modifier `src/`, contourner la licence (`elssrv`, SETINIT).
- Ajouter des modules NLS non-UTF-8 à `sas_small` (au plus, le signaler comme limite).

## Decisions

1. **Méthode différentielle avant décompilation.** Pour l'affichage et les catalogues, on génère d'abord des entrées contrôlées (valeurs limites, largeurs variées, catalogues ne différant que d'un élément) et on compare sorties/fichiers ; la décompilation REA (`tkefmt.so`, `tkefmf*.so`, `tkefmi*.so`, `tknlsenc.so`, `tknlsicv.so`) ne sert qu'à lever les ambiguïtés restantes et à l'inventaire. *Alternative écartée* : décompiler d'emblée les routines de formatage — plus coûteux, plus de risque juridique, et ne prouve pas le comportement observable. Conforme à la limite de l'exception d'interopérabilité (seules les informations non disponibles).
2. **Inventaire statique par tables de noms et symboles.** Les noms de formats sont extraits des chaînes/tables des modules plutôt que de la documentation SAS, puis recoupés avec la documentation publique. *Alternative* : partir de la documentation — incomplète pour les formats internes et ne donne pas le module.
3. **Priorisation : classification > affichage > catalogues > NLS.** La classification est ce dont le driver a besoin pour typer les colonnes ; l'affichage n'est spécifié que pour la liste courante du prompt. *Alternative* : spécifier tous les formats — disproportionné.
4. **Vérification par l'oracle comme seule preuve de comportement.** `PROC PRINT` et `CNTLOUT=` sont les références ; les specs publiques restent des hypothèses (`inféré`). *Alternative* : se fier à ReadStat/Parso — refusée par les conventions.
5. **Repli explicite pour la partie B.** Si `PROC FORMAT` échoue dans l'oracle, utiliser `corpus/external/` ; sinon marquer la partie bloquée avec la raison plutôt que d'extrapoler depuis `core.sas7bcat` (catalogue système, structure générique seulement).
6. **Environnement.** L'oracle et les outils REA tournent dans l'environnement Linux x86-64 établi par 01 ; les vérificateurs de `rea_output/tools/` sont des scripts autonomes (sans dépendance à SAS à l'exécution) qui comparent à des sorties d'oracle enregistrées dans `rea_output/evidence/`.
7. **Séparation analyse / conception.** Tout le matériel de décompilation reste sous `rea_output/evidence/` ; `30_formats_encodages.md` ne contient que des descriptions de format et d'algorithme (pseudo-code en anglais), afin que 06 puisse le lire sans voir de code SAS.

## Risks / Trade-offs

- [Licence SAS / interopérabilité] La licence de `src/` appartient à NULL ; l'analyse n'est couverte que pour l'interopérabilité et les informations non publiées. → Tâche initiale : consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte ; décompilation limitée aux manques identifiés par 00 ; aucun contournement `elssrv`/SETINIT.
- [Émulation x86-64 sur Mac] Sous Apple Silicon, l'émulation (Rosetta/QEMU) peut être lente ou instable, et des différences d'arrondi flottant sont théoriquement possibles. → Exécuter l'oracle dans l'environnement validé par 01 ; enregistrer les sorties de référence une fois et faire tourner les vérificateurs hors SAS ; signaler toute divergence suspecte dans `60_inconnues.md`.
- [Hypothèses publiques non vérifiées] Les tables d'encodages et de formats de ReadStat/Parso peuvent être incomplètes ou fausses. → Les marquer `inféré` tant qu'elles ne sont pas confirmées par binaire + fichier/runtime.
- [Oracle limité à UTF-8] Les codes d'encodage non UTF-8 ne peuvent être vérifiés au runtime. → Confiance `inféré` pour ces lignes, preuve binaire (`tknlsenc.so`) + fichiers du corpus ; limite consignée dans `STATE.md`.
- [`PROC FORMAT` indisponible dans `sas_small`] → Repli `corpus/external/` ou partie B marquée bloquée (Décision 5).
- [Arrondi de `BEST` / notation E difficile à reproduire exactement] → Corpus de valeurs limites dédié ; écarts résiduels listés dans `60_inconnues.md` plutôt que masqués.

## Migration Plan

Sans objet (documents de spécification uniquement). Retour arrière : les livrables sont des fichiers sous `rea_output/` ; `src/` n'est jamais modifié.

## Open Questions

- Le corpus de 01 contient-il des fichiers non UTF-8 (latin1, shift-jis…) ? Sinon, la couverture d'encodages reposera sur les sources publiques et la preuve binaire, sans changer les tâches.

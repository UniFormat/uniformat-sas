## Context

Voir `proposal.md` (Why) pour la motivation et `specs/sas7bdat-file-format/spec.md` pour les exigences.

État attendu au démarrage :
- `rea_output/00_etat_de_l_art.md` (changement `survey-sas7bdat-prior-art`) liste la structure publiquement documentée et les manques.
- `corpus/` et `corpus/MANIFEST.md` (changement `setup-sas-oracle-corpus`) fournissent des fichiers générés par paires ne différant que par un paramètre, des fichiers `external/`, et des CSV / sorties `PROC CONTENTS` de référence.
- `src/` est un sous-ensemble SAS 9.4 TS1M7 (ELF x86-64 Linux, UTF-8, binaires non strippés), en lecture seule ; l'hôte de travail est macOS.
- Aucun répertoire `rea_output/` ni `corpus/` n'existe encore dans le dépôt : ils sont produits par les changements 00 et 01.

## Goals / Non-Goals

**Goals:**
- Minimiser la part de décompilation : chaque champ est d'abord établi par les sources publiques puis par différentiel de fichiers.
- Rendre chaque affirmation rejouable (scripts dans `rea_output/tools/`, preuves dans `rea_output/evidence/`).
- Produire une spec autosuffisante, lisible par `design-rust-driver` sans accès aux preuves de décompilation.

**Non-Goals:**
- Écrire du code Rust ou un lecteur de production (`ref_reader.py` est un outil de vérification, non livrable produit).
- Performance du lecteur de référence.
- Variantes BE/32 bits au-delà de ce que le corpus `external/` permet d'observer (documentées en `inféré` si non observées).

## Decisions

### D1 — Méthode différentielle avant décompilation
Ordre : état de l'art → différentiel → décompilation ciblée → lecteur de référence.
- Pourquoi : l'exception d'interopérabilité (directive UE 2009/24, art. 6) ne couvre que les informations non déjà disponibles ; le différentiel est moins coûteux, rejouable et ne touche pas au code protégé.
- Alternative écartée : décompiler d'abord `sase7*` puis vérifier sur fichiers (approche de `prompts/archive/01_rea_sas7bdat_engine.md`) — trop large, juridiquement moins défendable, et favorise la recopie de logique SAS.

### D2 — Décompilation pilotée par les symboles
Les binaires n'étant pas strippés, l'analyse REA part des chaînes et symboles (`search_strings`, `search_procedures`) puis `analyze_function` / `trace_call_path` de l'ouverture d'un fichier jusqu'au décodage d'une ligne, limitée aux modules listés dans la spec.
- Alternative écartée : décompilation exhaustive des modules — coût élevé, hors du principe de ciblage.

### D3 — Lecteur de référence écrit depuis la seule spec
`ref_reader.py` en Python pur (bibliothèque standard), sans ReadStat/pandas/Parso, écrit en ne consultant que `10_format_sas7bdat.md`. Un écart corrige la spec d'abord.
- Pourquoi : c'est le test exécutable de la complétude de la spec, et il préfigure la séparation analyse/conception.
- Alternative écartée : comparer à ReadStat/pandas — ce serait valider la spec contre une autre hypothèse publique, pas contre SAS.

### D4 — Environnement d'exécution SAS
SAS (oracle de génération et de `PROC CONTENTS`) tourne dans l'environnement Linux x86-64 retenu par `setup-sas-oracle-corpus` ; ce changement ne le redéfinit pas. Les analyses REA statiques et les scripts Python tournent sur l'hôte macOS.
- Alternative : exécuter aussi REA dans le Linux — inutile pour l'analyse statique ; réservé à une éventuelle observation runtime.

### D5 — Séparation analyse / conception
Seule `rea_output/10_format_sas7bdat.md` (et `60_inconnues.md`) sera lue par `design-rust-driver` ; les sorties de décompilation restent confinées dans `rea_output/evidence/`. Les entrées de spec décrivent un comportement, citent `fonction@adresse`, sans pseudo-code.

### D6 — Organisation du livrable
Une entrée par champ, regroupées par structure (en-tête, page, sous-en-têtes, colonnes, valeurs, lignes supprimées, contrôles d'ouverture), offsets 32 et 64 bits côte à côte, variantes LE/BE signalées. Les inconnus restent dans `60_inconnues.md` avec renvoi croisé.

## Risks / Trade-offs

- [Licence SAS / interopérabilité : la décompilation de binaires NULL n'est couverte que pour l'interopérabilité et les informations non publiques] → tâche initiale : consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) ; décompilation limitée aux manques listés par `00_etat_de_l_art.md` ; aucun contournement de licence (`elssrv`, SETINIT) ; aucun code SAS recopié dans la spec.
- [Émulation x86-64 sur Mac (Apple Silicon) : SAS lent ou instable sous Rosetta/QEMU] → l'environnement est fixé par `setup-sas-oracle-corpus` ; ce changement n'exige SAS que pour régénérer des paires différentielles manquantes, en lots ; si SAS refuse de démarrer, arrêt et signalement dans `STATE.md`.
- [Hypothèses publiques non vérifiées prises pour acquises] → tout champ issu de l'état de l'art démarre en `inféré` et ne passe `confirmé` qu'avec une double preuve.
- [Corpus insuffisant pour isoler un paramètre (BE, 32 bits, types de pages rares comme amd)] → documenter en `inféré`/`inconnu`, ajouter une demande de fichier dans « Prochaine action » de `STATE.md` plutôt que de deviner.
- [Biais de l'auteur : la même session écrit la spec et `ref_reader.py`] → écrire le lecteur sans rouvrir `evidence/` ; corriger la spec en priorité à chaque écart.
- [Champs « réservés » sans variation observable] → valeur par défaut observée consignée dans la rubrique Écriture, confiance `inféré`, et entrée dans `60_inconnues.md`.

## Open Questions

- Disponibilité de fichiers `external/` en big-endian ou 32 bits : dépend de `setup-sas-oracle-corpus` ; sans eux, ces variantes restent `inféré` sans changer la méthode.

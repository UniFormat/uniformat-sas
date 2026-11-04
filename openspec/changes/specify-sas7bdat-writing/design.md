## Context

Voir `proposal.md` (Why) pour la motivation et `specs/sas7bdat-writing/spec.md` pour les exigences. Ce changement correspond au prompt 05 (`prompts/05_ecriture.md`) et suit les conventions de `prompts/README.md`.

État et contraintes qui façonnent l'approche :

- Entrées disponibles : `rea_output/10_format_sas7bdat.md` (`specify-sas7bdat-read-format`, 02), `rea_output/15_compression.md` et `rea_output/tools/ref_compress.py` (`specify-sas7bdat-compression`, 03), `rea_output/30_formats_encodages.md` s'il existe (`specify-sas-formats-encodings`, 04), les manques en écriture listés par `survey-sas7bdat-prior-art` (00) dans `rea_output/00_etat_de_l_art.md`, et l'oracle SAS + `corpus/` fournis par `setup-sas-oracle-corpus` (01).
- L'oracle `src/` (`sas_small`) est un sous-ensemble de SAS 9.4 TS1M7, ELF x86-64 Linux, UTF-8, en lecture seule, validé pour la **lecture** (`PROC CONTENTS`, `PROC PRINT`, étape DATA). C'est exactement l'usage requis ici : SAS sert de relecteur des fichiers produits, pas d'écrivain.
- Hôte macOS : SAS tourne dans un Linux x86-64 (conteneur, VM ou machine distante) mis en place par 01.
- Les fichiers SAS « équivalents » servant de modèle aux champs recopiés proviennent du `corpus/` généré par le SAS complet (01).

## Goals / Non-Goals

**Goals:**
- Une boucle produire → relire → comparer reproductible et scriptée, rejouable sur toute la matrice.
- Une table des champs sans aucun champ « recopié sans explication ».
- Un document `20_ecriture.md` autosuffisant, lisible par `design-rust-driver` (06) sans accès à `evidence/`.

**Non-Goals:**
- Identité octet à octet avec SAS.
- Qualité logicielle de `ref_writer.py` (performance, API) : c'est un outil d'analyse jetable, pas le driver.
- Implémentation Rust ou choix d'architecture du driver (réservés à 06).
- Reverse engineering approfondi des cas hors priorité (append, suppression, index, contraintes, chiffrement).

## Decisions

### D1. Méthode différentielle (production + mutation) avant décompilation
On construit d'abord des fichiers à partir de la spec, on les fait relire, puis on mute les champs recopiés pour les classer. La décompilation de `sase7opn` / `sase7io` n'intervient que pour un rejet que les mutations n'expliquent pas.
- *Pourquoi* : la mutation donne une preuve « runtime » directement exploitable, couvre le comportement observable, et limite la décompilation aux seules informations non disponibles autrement (cadre de l'exception d'interopérabilité).
- *Alternative écartée* : décompiler d'emblée le chemin d'écriture de SAS — plus coûteux, plus exposé juridiquement, et risque de transférer des détails d'implémentation SAS dans la spec.

### D2. SAS comme relecteur seulement, via l'environnement de 01
Les fichiers sont écrits par `ref_writer.py` (Python, hors SAS) et seulement **lus** par SAS dans le Linux x86-64 de `setup-sas-oracle-corpus`.
- *Pourquoi* : `sas_small` est validé en lecture ; l'écriture permanente par SAS n'est pas un objectif. Les fichiers de référence SAS existent déjà dans `corpus/`.
- *Alternative écartée* : générer de nouveaux fichiers SAS à la volée avec `sas_small` — non validé pour l'écriture ; si un fichier de référence manque, on le génère avec l'environnement complet de 01 et on l'ajoute au `corpus/MANIFEST.md`.

### D3. Écrivain de référence en Python, incrémental
`ref_writer.py` part du cas minimal (1 colonne num, 1 ligne, non compressé) et ajoute un cas validé à la fois, en réutilisant `ref_compress.py`.
- *Pourquoi* : isole chaque régression ; Python est déjà l'écosystème des lecteurs tiers (`pyreadstat`, pandas).
- *Alternative écartée* : écrire directement en Rust — violerait la séparation analyse/conception (06 conçoit à partir de la spec seule).

### D4. Spec corrigée avant l'écrivain
Tout écart de la matrice corrige d'abord `20_ecriture.md` (ou `10_format_sas7bdat.md` / `15_compression.md` si l'erreur y est), puis l'écrivain.
- *Pourquoi* : le livrable est la spec ; l'écrivain n'en est que la preuve exécutable.

### D5. Séparation analyse / conception
`20_ecriture.md` ne référence les preuves que par identifiant (`fonction@adresse`, fichier + offset, log) ; pseudo-code et noms de champs en anglais, sans extrait de code décompilé. Les détails bruts restent dans `rea_output/evidence/`.
- *Pourquoi* : 06 ne lit que les documents de spec.

### D6. Lecteurs et critère « lecteurs disponibles »
SAS, `pyreadstat` et pandas sont obligatoires ; Parso est utilisé s'il est installable (JVM). La liste des lecteurs effectivement utilisés est écrite en tête de `write_matrix.md`.

## Risks / Trade-offs

- [Licence SAS / interopérabilité] La licence appartient à NULL (utilisateur salarié, titulaire). L'exception de la directive UE 2009/24 art. 6 ne couvre que les informations non déjà disponibles. → Tâche initiale : consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte ; décompilation limitée aux rejets inexpliqués (D1) ; aucun contournement `elssrv` / SETINIT ; si SAS refuse de démarrer, arrêt et signalement.
- [Émulation x86-64 sur Mac] SAS sous émulation (Rosetta/QEMU) peut être lent ou instable, en particulier pour le cas > 2 Go et les nombreuses mutations. → Scripter la boucle pour la rejouer en lot ; privilégier une VM/machine distante x86-64 native si disponible ; marquer « non faisable » plutôt que « OK » si un cas ne peut tourner.
- [Hypothèses publiques non vérifiées] Les specs ReadStat/Parso/pandas/Shotwell peuvent être fausses ou incomplètes, et ces lecteurs sont plus tolérants que SAS. → SAS reste l'arbitre ; une acceptation par les lecteurs tiers seuls ne suffit jamais ; chaque règle porte un niveau de confiance.
- [Champ « libre » à tort] Un champ ignoré par SAS en lecture peut être utilisé ailleurs (autre PROC, append, autre version de SAS). → Classer `libre` uniquement avec la preuve de mutation, mentionner la portée testée (procédures, version 9.4 TS1M7) et lister le doute dans `60_inconnues.md`.
- [Encodage autre qu'UTF-8] `sas_small` est en UTF-8 ; relire un fichier latin1 déclenche un transcodage (CEDA) susceptible d'émettre des messages. → Distinguer `NOTE` de `WARNING`/`ERROR` dans le critère ; si un relecteur SAS dans l'encodage natif est nécessaire, le signaler comme blocage plutôt que modifier `src/`.
- [Gros volume > 2 Go] Coût disque et temps sous émulation. → Cas optionnel « si faisable », documenté dans la matrice dans tous les cas.
- [Dépendance aux changements 02/03] Une erreur de la spec de lecture ou de compression se propage. → D4 : corriger la spec source et le noter dans `STATE.md`.

## Open Questions

- Version de Parso et JVM disponibles dans l'environnement : n'affecte que la colonne Parso de la matrice.
- Faisabilité pratique du cas > 2 Go selon l'espace disque de l'environnement Linux retenu.

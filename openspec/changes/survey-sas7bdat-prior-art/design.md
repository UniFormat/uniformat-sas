## Context

Motivation : voir `proposal.md` (section Why). Exigences : voir `specs/sas7bdat-prior-art/spec.md`.

État actuel : aucun document dans `rea_output/`, aucune spec OpenSpec existante. `src/` contient un sous-ensemble de SAS 9.4 TS1M7 (ELF x86-64 Linux, UTF-8, licence Apache) qui servira d'oracle à partir de `setup-sas-oracle-corpus` (01), mais n'est pas utilisé ici. L'hôte de travail est macOS ; ce changement n'a besoin que d'un accès réseau en lecture (GitHub, crates.io, PyPI, CRAN, documentation SAS).

Contraintes : conventions de `prompts/README.md` (français, niveaux de confiance, sources publiques = hypothèses), et exception d'interopérabilité (directive UE 2009/24, art. 6) qui limite la décompilation aux informations non déjà disponibles.

## Goals / Non-Goals

**Goals:**
- Délimiter précisément le connu public, pour justifier et borner la décompilation des changements 02 à 05.
- Fournir à chaque changement de spécification une liste de manques priorisée (bloquant / dégradé / confort) avec une méthode.
- Fixer une règle de lecture par source compatible avec la licence visée pour le driver.

**Non-Goals:**
- Vérifier les descriptions publiques contre SAS (fait par 02 à 05 avec le corpus de 01).
- Choisir l'architecture Rust ou les crates (réservé à `design-rust-driver`, 06).
- Évaluer la qualité ou la performance des implémentations au-delà de la matrice de capacités.

## Decisions

1. **État de l'art avant tout reverse engineering.** On documente le connu public avant de toucher aux binaires. Alternative écartée : décompiler d'emblée le moteur V9 (approche de `prompts/archive/01_rea_sas7bdat_engine.md`) — trop large, contraire au cadre de l'art. 6 et redondant avec l'existant.

2. **Méthode différentielle prioritaire, décompilation en dernier recours.** Pour chaque manque, la méthode par défaut est l'analyse différentielle de fichiers produits par SAS (corpus de 01) ; la décompilation n'est proposée que si la variation contrôlée d'entrées ne suffit pas (logique interne de compression, champs dont la valeur ne varie pas de façon observable). Alternative écartée : décompilation systématique — plus coûteuse, juridiquement plus exposée, et produisant des connaissances plus difficiles à séparer de la conception.

3. **Sources lues à la source, pas de mémoire.** Licences, versions et capacités sont relevées dans les dépôts et registres au moment du travail, avec chemin + ligne ou section. Alternative écartée : synthèse à partir de connaissances générales — non vérifiable et inutilisable comme justification d'interopérabilité.

4. **Granularité de la matrice : capacités du format, pas API.** Une ligne par capacité imposée par le prompt 00, valeurs `oui / non / partiel / inconnu`. Alternative écartée : comparaison fonctionnelle des API — hors sujet pour la spec du format.

5. **Règle de lecture par licence.** Sources permissives (MIT, Apache-2.0, BSD) : lecture du code autorisée pendant la conception. Sources copyleft (GPL, LGPL) : lecture limitée à la documentation et aux comportements observables, pas de lecture détaillée du code par la personne qui conçoit le driver. Licence inconnue : traitée comme copyleft. La licence finale recommandée pour le driver est décidée dans le livrable, l'hypothèse de travail étant une licence permissive (MIT/Apache-2.0) adaptée à l'exposition R/Python et à une intégration Dremio/Arrow Flight.

6. **Séparation analyse / conception préparée dès 00.** Le livrable ne contient ni pseudo-code Rust ni choix d'architecture, pour que `design-rust-driver` (06) puisse le lire sans hériter de décisions d'implémentation ; seules les règles de licence lui sont transmises.

7. **Choix d'environnement.** Aucun environnement SAS n'est requis ici ; la question Linux x86-64 (conteneur, VM ou machine distante) est laissée à `setup-sas-oracle-corpus` (01). Le livrable peut signaler les manques qui imposent des fichiers produits sur des plateformes non disponibles (big endian, 32 bits) pour que 01 en tienne compte.

## Risks / Trade-offs

- [Licence SAS / interopérabilité] La décompilation ultérieure n'est couverte que pour les informations non publiques et dans le cadre de la licence Apache → ce changement trace précisément le connu public ; les changements 02 à 05 commencent par consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte. Ce changement-ci, purement documentaire, n'en dépend pas, mais `STATE.md` doit lister cette confirmation comme blocage ouvert pour 02.
- [Hypothèses publiques non vérifiées] Les implémentations peuvent se copier entre elles et partager les mêmes erreurs → toute affirmation est marquée `inféré` au mieux, les désaccords sont explicités, et la vérification est renvoyée à 02–05.
- [Contamination de licence] Lire en détail une source GPL rendrait délicat un driver sous licence permissive → règle de lecture par source (décision 5) appliquée dès ce changement, notamment pour ReadStat et le paquet R `sas7bdat` selon les licences effectivement constatées.
- [Émulation x86-64 sur Mac] Sans incidence ici, mais conditionne 01 (SAS sous émulation Rosetta/QEMU potentiellement lente ou instable) → signalé dans `STATE.md` comme point d'attention pour `setup-sas-oracle-corpus`.
- [Obsolescence] Versions et activité évoluent → date de consultation notée pour chaque entrée.
- [Couverture incomplète] Des crates ou paquets mineurs peuvent échapper à la recherche → requêtes effectuées (crates.io, PyPI, npm, pkg.go.dev, GitHub) consignées dans la section 1.

## Open Questions

- Licence finale du driver (MIT, Apache-2.0 ou double licence) : la recommandation est produite par ce changement, la décision définitive peut être confirmée en 06 sans modifier les specs ni les tâches.

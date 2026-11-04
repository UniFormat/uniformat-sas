## Context

Voir `proposal.md` (Why). Au démarrage de ce changement, `rea_output/`, `corpus/` et `docs/` n'existent pas encore dans le dépôt : ils sont produits par les changements amont `survey-sas7bdat-prior-art` (00), `setup-sas-oracle-corpus` (01), `specify-sas7bdat-read-format` (02), `specify-sas7bdat-compression` (03), `specify-sas-formats-encodings` (04) et `specify-sas7bdat-writing` (05). Le prompt 06 est donc exécutable seulement après 02 à 05.

Contraintes structurantes :
- Séparation analyse / conception (`prompts/README.md`) : la conception ne voit que les documents de spécification, jamais `src/`, `rea_output/evidence/` ni une sortie de décompilation. Ce n'est pas une salle blanche formelle (même utilisateur, même agent), mais la traçabilité des entrées est exigée.
- Une session neuve, sans skill REA, ouverte à la racine `UniFormat/`.
- Hôte macOS ; SAS (oracle des tests de round-trip) ne tourne que dans un Linux x86-64.
- Documents en français, identifiants et signatures en anglais.

## Goals / Non-Goals

**Goals:**
- Fixer, avant tout code, les décisions qui coûtent cher à changer (frontière du cœur, modèle d'I/O, mapping des types, missing spéciaux, sémantique SQL, écriture, inconnus).
- Garantir que chaque section de la spec et chaque inconnu a une place et un comportement dans le driver.
- Ordonner l'implémentation en jalons testables sur le corpus.

**Non-Goals:**
- Écrire du Rust, des bindings ou des scripts ; créer un workspace Cargo.
- Compléter ou corriger la spec du format (on consigne un manque, on ne le comble pas ici).
- Choisir l'infrastructure de déploiement Flight/Dremio au-delà de l'interface (plages de pages, schéma seul).

## Decisions

### D1 — Conception à partir de la seule spécification (séparation analyse/conception)
Le driver est conçu d'après `rea_output/10`, `15`, `20`, `30`, `60` et `corpus/MANIFEST.md`. Alternative écartée : concevoir en parallèle de l'analyse ou en consultant les preuves de décompilation — plus rapide, mais le code Rust risquerait de refléter la structure du code SAS, ce qui affaiblit la position d'interopérabilité et mélange preuves et choix d'ingénierie. Alternative écartée : s'inspirer du code ReadStat/Parso — autorisé seulement si la section licences de `00` le permet (Parso est sous GPL ; ReadStat et pandas sous licences permissives, à confirmer par 00).

### D2 — Une ADR par décision, plus une architecture et une roadmap
Les sept décisions du prompt deviennent chacune une ADR `docs/adr/NNNN-*.md` (format contexte / options / décision / conséquences). Alternative écartée : un seul document de conception — plus difficile à réviser et à faire évoluer décision par décision.

### D3 — Orientations attendues des ADR (à confirmer dans la session, non figées ici)
- Crates : cœur `sas7bdat` sans I/O (décodage/encodage sur `&[u8]` / tampons), adaptateurs `-arrow`, `-datafusion`, `-py`, `-r`, `-cli`. Alternative : crate unique avec features — plus simple au début, mais tire Arrow/async dans tous les usages et complique le fuzzing.
- I/O : trait synchrone `Read + Seek` pour le cœur, lecture par plages de pages (offset, longueur dérivés de l'en-tête) pour permettre mmap et `object_store` ; l'async reste dans les adaptateurs. Alternative : cœur async — imposerait un runtime aux bindings R/Python.
- Missing spéciaux : comparer colonne annexe, métadonnée Arrow et type extension ; le choix doit permettre la restitution `haven` (R) et pandas/polars (Python) sans perte.
- Sémantique SQL : choix propres au driver, documentés comme tels, non déduits du binaire.
- Inconnus : par défaut tolérance + avertissement en lecture, erreur en écriture sauf valeur par défaut classée `constante` dans `20` ; à ajuster inconnu par inconnu.

### D4 — Tests ancrés sur le corpus et sur des lecteurs indépendants
Le corpus `corpus/` (CSV de référence) sert de test d'intégration ; le round-trip implique SAS, ReadStat et pandas ; proptest et `cargo-fuzz` couvrent décodage et décompression. Alternative écartée : tests unitaires seuls — ne prouvent pas l'interopérabilité.

### D5 — Exécution de SAS hors de la session de conception
La conception ne lance pas SAS. Les tests de round-trip par SAS sont décrits pour plus tard et s'exécutent dans l'environnement Linux x86-64 défini par `setup-sas-oracle-corpus`.

## Risks / Trade-offs

- [Licence SAS / interopérabilité] La spec amont résulte d'analyses du binaire SAS sous licence Apache ; l'utilisateur travaille chez NULL, titulaire de la licence. → Vérifier au démarrage que `rea_output/STATE.md` contient la confirmation interne (référent licence SAS / juridique NULL) consignée par les changements 02 à 05 ; ne pas lire les preuves de décompilation ; limiter la conception à une description du format.
- [Licence du driver] Une lecture non autorisée de code GPL (Parso) contaminerait la licence. → Ne lire du code tiers que si `00` l'autorise ; l'ADR licence justifie le choix (ex. MIT/Apache-2.0) par `00`.
- [Hypothèses publiques non vérifiées] Des éléments de ReadStat/Parso/pandas/Shotwell peuvent subsister en `inféré` dans la spec. → Les reporter avec leur niveau de confiance dans les ADR ; tout point `inféré` ou `inconnu` reçoit un comportement défensif (ADR inconnus).
- [Émulation x86-64 sur Mac] Les tests round-trip par SAS dépendent d'un Linux x86-64 (émulation lente ou machine distante). → Les isoler en suite optionnelle ; le jalon de chaque étape repose d'abord sur le corpus et sur ReadStat/pandas.
- [Spec amont incomplète] Une section de `10`/`15`/`20`/`30` absente bloque la traçabilité. → Arrêt et consignation dans `STATE.md`, renvoi vers le changement amont.
- [Séparation imparfaite] Même agent et même utilisateur que l'analyse. → Session neuve, entrées listées dans `docs/architecture.md`.
- [Fichiers hostiles] Parseur exposé à des fichiers non fiables (S3, Flight). → Allocations bornées, `unsafe` interdit ou circonscrit, fuzzing dès le premier jalon.

## Open Questions

- Licence finale du driver et politique de publication (interne NULL ou open source) : dépend de `00` et d'une décision interne, sans impact sur la structure des ADR.
- Choix précis de MSRV : fixé dans l'ADR selon les versions d'Arrow/DataFusion disponibles au moment de la session.

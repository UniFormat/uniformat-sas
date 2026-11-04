## 1. Préparation de la session (session neuve, sans REA)

- [ ] 1.1 Lire `prompts/README.md` puis `rea_output/STATE.md` ; vérifier que 02 à 05 sont marqués terminés et que la confirmation interne NULL (référent licence SAS / juridique) y est consignée — sinon consigner le blocage dans `STATE.md` et s'arrêter — NON SATISFAIT : 02 à 05 sont `partiel` et la confirmation NULL est absente ; poursuite sur instruction explicite de l'utilisateur (« Oui, lance 06 »), blocage maintenu dans `STATE.md`.
- [x] 1.2 Vérifier la présence de `rea_output/00_etat_de_l_art.md`, `10_format_sas7bdat.md`, `15_compression.md`, `20_ecriture.md`, `30_formats_encodages.md`, `60_inconnues.md` et `corpus/MANIFEST.md` (liste `ls`) ; tout manquant → blocage dans `STATE.md` avec le changement amont à terminer — les sept documents sont présents.
- [ ] 1.3 Lire ces sept documents et uniquement eux (aucun accès à `src/`, `rea_output/evidence/`, sorties de décompilation) ; noter la section licences de `00` pour savoir si du code tiers peut être consulté — lecture ciblée (sections utiles et titres) des documents volumineux, `src/`, `evidence/` et décompilation non lus ; la section licences de `00` (5.1) lue en entier : aucun code tiers n'a été lu.
- [x] 1.4 Dresser l'inventaire des sections de `10`, `15`, `20`, `30` et des entrées de `60` (liste de travail utilisée pour les tables de traçabilité et de couverture) — inventaire des sections de `10`, `15`, `20`, `30` et des 55 entrées de `60` : table de traçabilité de `docs/architecture.md` §5 et ADR 0006.

## 2. Architecture

- [x] 2.1 Créer `docs/architecture.md` avec la liste des documents d'entrée lus et la vue d'ensemble ; vérifier que chaque entrée listée appartient à la liste autorisée — `docs/architecture.md` §0.1.
- [x] 2.2 Décrire le découpage en crates (`sas7bdat` cœur, `-arrow`, `-datafusion`, `-py`, `-r`, `-cli`) et le graphe de dépendances ; vérifier qu'aucune flèche ne part du cœur vers un adaptateur — §2.
- [x] 2.3 Décrire le flux de données fichier → en-tête → pages → lignes → lots Arrow → consommateurs (DataFusion, Python, R, Flight) — §3.
- [x] 2.4 Esquisser l'API publique (signatures seulement) : ouverture, métadonnées complètes, schéma sans lecture des données, itération par lots, lecture par plage de pages, écriture ; vérifier que les cinq usages du prompt ont chacun une signature — §4 (signatures, cinq usages en 4.6).
- [x] 2.5 Ajouter la table de traçabilité section de spec (`10`, `15`, `20`, `30`) → module/crate ; vérifier qu'aucune section de l'inventaire 1.4 n'est absente — §5, vérifié par script (chaque section et sous-section de `10`, `15`, `20`, `30` a une ligne).

## 3. ADR

- [x] 3.1 `docs/adr/0001-*.md` découpage en crates et dépendances autorisées dans le cœur (au moins deux options comparées) — `0001-decoupage-en-crates.md`.
- [x] 3.2 `docs/adr/0002-*.md` modèle d'I/O : `Read + Seek`, mmap, `object_store` S3/HTTP, lecture par plages, async ou non — `0002-modele-d-io.md`.
- [x] 3.3 `docs/adr/0003-*.md` mapping des types depuis la table de `30` (Float64/Date32/Timestamp/Time64/Decimal ; Utf8 et option blancs finaux ; missing → null) et choix justifié du mécanisme de conservation des missing spéciaux, avec restitution `haven` (R) et pandas/polars (Python) — `0003-mapping-des-types-et-missing.md`.
- [x] 3.4 `docs/adr/0004-*.md` sémantique SQL du driver : comparaison avec espaces finaux, ordre des missing, lignes supprimées invisibles, projection/`LIMIT` poussés, lecture parallèle par plages, statistiques — `0004-semantique-sql.md`.
- [x] 3.5 `docs/adr/0005-*.md` écriture : API depuis Arrow et depuis un itérateur de lignes, compression (aucune/`CHAR`/`BINARY`), validation des limites de `20` avant écriture, comportement sur valeur non représentable — `0005-ecriture.md`.
- [x] 3.6 `docs/adr/0006-*.md` gestion des inconnus : table inconnu de `60` → comportement lecture / écriture (erreur, avertissement, tolérance + valeur par défaut) ; vérifier que chaque entrée de `60` y figure — `0006-gestion-des-inconnues.md` : 55 entrées, vérifié par script.
- [x] 3.7 `docs/adr/0007-*.md` erreurs (`thiserror`), `unsafe` interdit ou circonscrit, MSRV, licence du driver justifiée par la section licences de `00` — `0007-erreurs-unsafe-msrv-licence.md` (licence justifiée par `00` §5.2).
- [x] 3.8 Vérifier que `docs/adr/` contient exactement une ADR par décision et que chaque ADR a contexte, options, décision, conséquences — 7 ADR, chacune avec contexte, options (au moins deux), décision, conséquences (vérifié par script).

## 4. Stratégie de tests

- [x] 4.1 Rédiger dans `docs/architecture.md` la section tests d'intégration sur `corpus/` (comparaison aux CSV de référence de `corpus/MANIFEST.md`) — `docs/architecture.md` §6.1.
- [x] 4.2 Décrire le round-trip écriture → lecture par le driver, SAS (environnement Linux x86-64 de 01, suite optionnelle), ReadStat et pandas — §6.2.
- [x] 4.3 Décrire les tests par propriétés (proptest) sur le décodage des valeurs et la compression — §6.3.
- [x] 4.4 Décrire le fuzzing `cargo-fuzz` (cibles : en-tête, pages, décompresseur ; propriétés : aucune panique, allocations bornées) — §6.4.
- [x] 4.5 Décrire les benchmarks contre ReadStat/pyreadstat et pandas sur le même corpus — §6.5.

## 5. Feuille de route

- [x] 5.1 Créer `docs/roadmap.md` avec les jalons dans l'ordre lecture non compressée → compression → métadonnées/formats → Arrow/DataFusion → Python/R → écriture → Flight — `docs/roadmap.md`.
- [x] 5.2 Donner à chaque jalon un critère de fin mesurable (tests du corpus, round-trip, benchmarks) et les ADR qu'il met en œuvre — critère de fin et ADR par jalon.

## 6. Vérification et clôture

- [x] 6.1 Vérifier le critère de fin : toutes les sections de `10`, `15`, `20`, `30` rattachées ; tous les inconnus de `60` avec un comportement défini — traçabilité complète et 55 inconnues avec comportement.
- [x] 6.2 Vérifier qu'aucun code d'implémentation n'a été écrit (aucun `Cargo.toml`, `.rs`, `.py`, `.R`) et que seuls `docs/` et `rea_output/STATE.md` ont changé — aucun `Cargo.toml`, `.rs`, `.py`, `.R` créé.
- [x] 6.3 Mettre à jour `rea_output/STATE.md` (format de `prompts/README.md`) : prompt 06, date, résultat, avancement, blocages ouverts, une décision par ADR, prochaine action (jalon 1 de la roadmap) — `STATE.md` mis à jour.

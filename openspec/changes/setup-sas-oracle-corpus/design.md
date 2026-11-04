## Context

Voir `proposal.md` (Why) pour la motivation. État actuel et contraintes :

- `src/sas_small` est un sous-ensemble de SAS Foundation 9.4 TS1M7 (ELF x86-64 Linux, UTF-8, ~113 Mo), validé uniquement pour la **lecture** (`PROC CONTENTS`, `PROC PRINT`, étape DATA). L'écriture d'une table permanente, `PROC FORMAT`, `PROC SQL`, `PROC EXPORT` et les encodages autres qu'UTF-8 ne sont pas garantis (modules NLS latin1 absents).
- Le lanceur est relocatable et écrit `WORK`/`SASUSER` sous `${TMPDIR:-/tmp}/sas_small.$USER`, ce qui permet de garder `src/` en lecture seule.
- Il embarque `elssrv` et s'appuie sur le SETINIT de la licence SAS de NULL ; l'utilisateur travaille chez NULL, titulaire de cette licence.
- L'hôte est un Mac (probablement Apple Silicon) : aucune exécution native possible.
- Les binaires visent GNU/Linux 2.6.18 : une glibc récente est en principe rétrocompatible, mais les bibliothèques partagées attendues doivent être présentes dans l'invité.
- Les conventions communes (`prompts/README.md`) imposent `rea_output/STATE.md` lu en premier et mis à jour en dernier, et la séparation analyse / conception.

## Goals / Non-Goals

**Goals:**
- Un oracle reproductible, lancé par une seule commande documentée depuis l'hôte.
- Un corpus organisé pour la **méthode différentielle** : chaque paire de fichiers isole un paramètre.
- Un manifeste suffisant pour que `design-rust-driver` (06) et les tests du driver utilisent le corpus sans relire les programmes de génération.

**Non-Goals:**
- Aucune lecture ni interprétation des octets des `.sas7bdat` (relève de 02 à 05).
- Aucune décompilation ni usage de REA.
- Aucune extension de `sas_small` (ajout de modules NLS, de PROC) : si une capacité manque, on la contourne fonctionnellement ou on bascule sur le SAS complet d'origine.
- Aucun outillage d'intégration continue autour du corpus.

## Decisions

### D1 — Constituer l'oracle et le corpus avant toute décompilation
La méthode retenue pour les phases suivantes est **différentielle d'abord** : comparer des fichiers qui ne diffèrent que par un paramètre isole la zone d'octets concernée, et réduit la décompilation aux seules zones encore inconnues après 00 et la comparaison.
- Alternative écartée : décompiler d'emblée les modules d'E/S V9 de `sasexe/`. Plus coûteux, moins vérifiable, et plus difficile à justifier au regard de l'exception d'interopérabilité (directive UE 2009/24, art. 6), qui ne couvre que les informations non déjà disponibles.

### D2 — Ordre de préférence des environnements, choix laissé à l'utilisateur
Ordre de présentation imposé : (1) Docker `--platform linux/amd64` (léger, scriptable, `src/` monté en lecture seule via `-v ...:/sas:ro`) ; (2) VM Linux x86-64 UTM/Parallels (isolation complète, nom d'hôte stable) ; (3) machine Linux distante (performances natives, dépend d'un accès) ; (4) SAS complet d'origine (référence ultime, hors poste).
- Le conteneur est privilégié pour sa reproductibilité, mais le nom d'hôte variable d'un conteneur peut gêner la licence : `--hostname` fixe est à prévoir. Le choix final revient à l'utilisateur (aucune installation avant sa réponse).
- Alternative écartée : installer automatiquement l'option la plus probable. Contraire au prompt et risqué (Docker Desktop, Rosetta, hyperviseur sont des installations système).

### D3 — Un programme par famille, une table de référence par famille
Chaque famille a un fichier `corpus/gen/<famille>.sas` qui produit une table de base puis des variantes ne changeant qu'un paramètre. Les variantes de compression sont produites systématiquement pour chaque table (option de data set `COMPRESS=`), plutôt qu'en famille séparée seulement, afin de satisfaire le critère de fin. Nommage proposé : `<famille>_<variante>_<compress>.sas7bdat`, avec `.contents.txt`, `.print.txt` et `.csv` associés.
- Alternative écartée : un programme unique géant. Moins lisible, et un échec bloque tout le corpus.

### D4 — CSV de référence indépendant de `PROC EXPORT`
Si `PROC EXPORT` est absent (probable dans `sas_small`), le CSV est écrit par une étape DATA (`put` avec `best32.` pour les numériques, missings spéciaux écrits tels quels). On utilise la même méthode partout si l'une des deux n'est pas disponible pour toutes les tables, afin que les CSV soient homogènes.

### D5 — Manifeste généré par script
`corpus/MANIFEST.md` est produit par un script hôte (taille + `shasum -a 256`) à partir d'une table de métadonnées (famille, paramètre varié, source, outil) tenue à côté des programmes ; les cas manquants y sont ajoutés comme lignes explicatives. Cela garantit la cohérence SHA-256 / fichiers.

### D6 — Séparation analyse / conception préservée
Le corpus et son manifeste sont des **données**, pas des analyses : ils font partie des documents autorisés pour `design-rust-driver` (06). Aucune observation sur les octets n'est écrite dans `corpus/` ni dans `rea_output/` par ce changement.

## Risks / Trade-offs

- [Licence SAS / interopérabilité] L'exécution de `sas_small` hors de l'infrastructure d'origine (poste Mac, conteneur, VM) peut ne pas être couverte par les conditions de la licence Apache, et le SETINIT peut refuser un nom d'hôte différent. → L'utilisateur, employé de NULL, consigne dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'usage comme oracle hors serveur d'origine est autorisé ; en cas de refus technique de la licence, arrêt et signalement, sans contournement ; repli sur le SAS complet d'origine. La même confirmation, étendue à l'analyse à des fins d'interopérabilité, sera exigée en tâche initiale des changements 02 à 05.
- [Émulation x86-64 sur Mac] Sous Rosetta ou qemu, SAS peut être lent, et certaines instructions ou appels système peuvent échouer (qemu en particulier). → Tester d'abord le programme minimal ; en cas d'échec propre à l'émulation, passer à l'option suivante (VM x86-64 complète ou machine distante).
- [Compatibilité glibc / bibliothèques] Binaires visant Linux 2.6.18 : des bibliothèques manquantes dans une image minimale (`libnsl`, `libstdc++` 32/64, etc.) peuvent empêcher le démarrage. → Choisir une image de base large (par exemple RHEL/Rocky compatible) et consigner les paquets ajoutés.
- [Capacités limitées de `sas_small`] L'écriture permanente, `PROC FORMAT`, les encodages latin1 ou `PROC DATASETS` peuvent échouer. → Inventaire explicite dans `STATE.md` ; familles concernées justifiées dans `MANIFEST.md` ou générées avec le SAS complet d'origine (outil de génération noté par fichier).
- [Hypothèses publiques non vérifiées] Les fichiers de `corpus/external/` et la documentation publique (ReadStat, Parso, pandas, Shotwell) peuvent être corrompus, atypiques ou mal étiquetés. → Ils sont marqués comme source externe dans le manifeste et restent des hypothèses à vérifier, jamais des preuves ; leur licence est notée avant copie.
- [Déterminisme] Les `.sas7bdat` contiennent des horodatages et éventuellement le nom d'hôte : deux générations ne sont pas identiques octet pour octet. → Le SHA-256 identifie la version figée du corpus ; la régénération n'est pas censée reproduire les mêmes empreintes, ce qui sera documenté dans le manifeste.
- [Données sensibles] Le corpus ne doit contenir que des données synthétiques. → Aucune table NULL réelle n'est copiée dans `corpus/`.

## Migration Plan

Sans objet : création de nouveaux répertoires (`corpus/`) et mise à jour de `rea_output/STATE.md`. Retour arrière : suppression de `corpus/` et de l'environnement d'exécution créé (conteneur, VM).

## Open Questions

- Architecture exacte de l'hôte (`arm64` attendu) et disponibilité préalable de Docker, UTM ou Parallels : détectées en début de session, sans incidence sur les specs.
- Disponibilité d'un SAS complet d'origine pour les familles que `sas_small` ne sait pas produire (latin1, `PROC FORMAT`) : réponse attendue de l'utilisateur au moment de l'inventaire.

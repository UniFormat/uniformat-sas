## Purpose

Disposer d'un oracle SAS exécutable (`sas_small` ou SAS complet d'origine) depuis l'hôte macOS, lancé par une commande documentée et dans le strict respect de la licence, afin de générer des fichiers `.sas7bdat` de référence et de relire ceux que le driver écrira.

## ADDED Requirements

### Requirement: Présentation ordonnée des options d'exécution
Le travail SHALL détecter l'architecture de l'hôte (`uname -m` : `arm64` ou `x86_64`) puis présenter les options d'exécution dans cet ordre : conteneur Docker `--platform linux/amd64` (Rosetta ou qemu), VM Linux x86-64 (UTM, Parallels), machine Linux distante, SAS complet d'origine. Chaque option MUST indiquer la commande exacte et ses limites (performances sous émulation, compatibilité glibc avec des binaires visant GNU/Linux 2.6.18).

#### Scenario: Hôte Apple Silicon
- **WHEN** `uname -m` renvoie `arm64`
- **THEN** les quatre options sont présentées dans l'ordre imposé, chacune avec sa commande exacte, et la limite liée à l'émulation x86-64 (Rosetta ou qemu) est explicitement signalée

#### Scenario: Option sans commande ou sans limites
- **WHEN** une option est présentée sans commande exacte ou sans limites
- **THEN** la présentation est considérée incomplète et doit être corrigée avant de solliciter le choix de l'utilisateur

### Requirement: Choix validé par l'utilisateur avant installation
Le travail MUST NOT effectuer d'installation (image Docker, VM, paquet, accès distant) avant que l'utilisateur ait explicitement choisi l'option à utiliser.

#### Scenario: Attente du choix
- **WHEN** les options ont été présentées et que l'utilisateur n'a pas encore répondu
- **THEN** aucune commande d'installation n'est exécutée et le travail reste en attente du choix

#### Scenario: Choix consigné
- **WHEN** l'utilisateur choisit une option
- **THEN** l'option retenue et sa justification sont consignées dans `rea_output/STATE.md` (section « Décisions prises »)

### Requirement: Exécution depuis une commande documentée
L'oracle SHALL exécuter un programme SAS minimal (`libname` + `PROC CONTENTS` + `PROC PRINT` sur une table fournie, ou `data _null_; put "ok"; run;`) depuis une commande documentée et reproductible, avec log et sortie listing conservés.

#### Scenario: Programme minimal réussi
- **WHEN** la commande documentée lance `sas_small` (ou le SAS complet) sur le programme minimal
- **THEN** le log ne contient aucune ligne `ERROR`, la sortie attendue (`ok` ou le listing CONTENTS/PRINT) est produite, et la commande exacte figure dans `rea_output/STATE.md`

### Requirement: Respect strict de la licence
En cas d'échec de démarrage lié à la licence (`elssrv`, SETINIT, nom d'hôte), le travail MUST s'arrêter et le signaler, sans aucune tentative de contournement. L'alternative autorisée est le SAS complet d'origine sous licence Apache.

#### Scenario: Refus de licence
- **WHEN** le log de démarrage signale une erreur de licence (SETINIT expiré, `elssrv` en échec, nom d'hôte non reconnu)
- **THEN** le travail s'arrête, le message exact est consigné dans `rea_output/STATE.md` (section « Blocages ouverts ») et aucune modification de `elssrv`, du SETINIT, du nom d'hôte ou des binaires n'est tentée

### Requirement: Lecture seule de src
Le répertoire `src/` MUST rester en lecture seule : `sas_small` y est exécuté mais aucun fichier n'y est créé, modifié ou supprimé.

#### Scenario: Vérification après exécution
- **WHEN** l'oracle a été exécuté une ou plusieurs fois
- **THEN** le contenu de `src/` est inchangé (les répertoires `WORK`/`SASUSER` se trouvent hors de `src/`)

### Requirement: Inventaire des capacités de l'oracle
Le travail SHALL tester et consigner dans `rea_output/STATE.md`, pour chacune des capacités suivantes, le résultat (fonctionne / échoue avec message) : écriture d'une table permanente (`data lib.t; ... run;`), `COMPRESS=CHAR` et `COMPRESS=BINARY`, `PROC FORMAT` avec `FMTLIB`, `PROC SQL`, `PROC EXPORT`, `ENCODING=` en sortie, `PROC DATASETS` (index, suppression de lignes via `MODIFY`/`REMOVE`).

#### Scenario: Capacité absente
- **WHEN** une capacité testée échoue (par exemple `PROC EXPORT` absent de `sas_small`)
- **THEN** l'échec et le message du log sont consignés dans `rea_output/STATE.md`, et le contournement fonctionnel prévu (par exemple CSV via étape DATA) y est indiqué

#### Scenario: Inventaire complet
- **WHEN** la phase d'inventaire est terminée
- **THEN** chaque capacité de la liste a un statut explicite dans `rea_output/STATE.md`, aucune n'étant laissée sans résultat

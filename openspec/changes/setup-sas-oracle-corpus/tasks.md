## 1. Préparation de session

- [x] 1.1 Lire `prompts/README.md`, `rea_output/STATE.md` et `rea_output/00_etat_de_l_art.md` (livrable de `survey-sas7bdat-prior-art`) ; vérifier que le changement 00 est marqué terminé dans `STATE.md`, sinon consigner le blocage
- [x] 1.2 Relever dans `00_etat_de_l_art.md` les manques à cibler par le corpus (familles, paramètres) et les noter comme liste de contrôle pour la section 5
- [x] 1.3 Consigner dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'exécution de `sas_small` comme oracle hors serveur d'origine est autorisée ; vérifier que la décision figure dans « Décisions prises »

## 2. Choix de l'environnement d'exécution

- [x] 2.1 Exécuter `uname -m` (et `sw_vers`) sur l'hôte, puis vérifier la présence de Docker, UTM, Parallels ; noter le résultat
- [x] 2.2 Rédiger les quatre options dans l'ordre imposé (Docker `--platform linux/amd64`, VM Linux x86-64 UTM/Parallels, machine Linux distante, SAS complet d'origine), chacune avec commande exacte (dont montage `src/` en lecture seule et `--hostname` fixe pour Docker) et limites (émulation, glibc / Linux 2.6.18) ; vérifier qu'aucune option n'est sans commande ni limite
- [x] 2.3 Demander à l'utilisateur quelle option utiliser et attendre sa réponse sans rien installer ; consigner le choix dans `rea_output/STATE.md`

## 3. Mise en route de l'oracle

- [x] 3.1 Installer et configurer l'option choisie (image de base, bibliothèques requises, montage de `src/` en lecture seule) ; vérifier que `src/sas_small` est visible et exécutable dans l'invité
- [x] 3.2 Écrire la commande de lancement documentée (wrapper hôte ou ligne exacte) et l'ajouter à `rea_output/STATE.md`
- [x] 3.3 Lancer le programme minimal `data _null_; put "ok"; run;` ; vérifier `ok` dans le log et l'absence de ligne `ERROR`
- [x] 3.4 Lancer `libname` + `PROC CONTENTS` + `PROC PRINT` sur une table fournie ; vérifier le listing et l'absence d'`ERROR`
- [x] 3.5 En cas d'erreur de licence (`elssrv`, SETINIT, nom d'hôte) : arrêter, consigner le message exact dans « Blocages ouverts » de `STATE.md`, ne rien contourner, proposer le repli sur le SAS complet d'origine
- [x] 3.6 Vérifier que `src/` est inchangé après les exécutions (comparaison avec `src/files_manifest.txt` ou listing horodaté)

## 4. Inventaire des capacités de l'oracle

- [x] 4.1 Tester l'écriture d'une table permanente (`data lib.t; ... run;`) et noter le résultat dans `STATE.md`
- [x] 4.2 Tester `COMPRESS=CHAR` et `COMPRESS=BINARY` sur une table permanente ; vérifier via `PROC CONTENTS` que la compression est appliquée
- [x] 4.3 Tester `PROC FORMAT` avec `FMTLIB` et noter le résultat
- [x] 4.4 Tester `PROC SQL` (création de table) et noter le résultat
- [x] 4.5 Tester `PROC EXPORT` en CSV ; si absent, noter le repli par étape DATA (`put` + `best32.`)
- [x] 4.6 Tester `ENCODING=` en sortie (UTF-8, latin1, wlatin1) et noter quels encodages fonctionnent
- [x] 4.7 Tester `PROC DATASETS` : création d'index et suppression de lignes via `MODIFY`/`REMOVE` ; noter le résultat
- [x] 4.8 Vérifier que chaque capacité 4.1 à 4.7 a un statut explicite (fonctionne / échoue + message) dans `rea_output/STATE.md`

## 5. Programmes de génération du corpus

- [x] 5.1 Créer l'arborescence `corpus/gen/`, `corpus/external/` et le dossier de sortie des fichiers générés ; fixer la convention de nommage `<famille>_<variante>_<compress>` (`.sas7bdat`, `.contents.txt`, `.print.txt`, `.csv`)
- [x] 5.2 Écrire une macro ou un include commun qui, pour une table donnée, produit les variantes `NO`/`CHAR`/`BINARY`, la sortie `PROC CONTENTS`, la sortie `PROC PRINT` et le CSV (via `PROC EXPORT` ou étape DATA selon 4.5) ; vérifier sur une table de test que les quatre fichiers sont produits
- [x] 5.3 Écrire `corpus/gen/base.sas` : 0, 1, 2, 100, 3 000 lignes ; 1 num ; 1 char ; mélange
- [x] 5.4 Écrire `corpus/gen/types.sas` : longueurs num 3 à 8 ; longueurs char 1, 8, 200, 32 767
- [x] 5.5 Écrire `corpus/gen/valeurs.sas` : missing `.`, `._`, `.A` à `.Z`, infinis/NaN si SAS en produit, valeurs extrêmes ; vérifier dans le CSV que les missings spéciaux sont écrits tels qu'affichés
- [x] 5.6 Écrire `corpus/gen/metadonnees.sas` : noms 1 et 32 caractères, labels vides / 256 caractères / non ASCII, label de table, formats et informats `date9.`, `datetime19.`, `time8.`, `dollar12.2`, `best12.`, `comma10.2`, `percent8.1`, `$char20.`, `e10.`
- [x] 5.7 Écrire `corpus/gen/taille.sas` : tables multipages et table à très nombreuses colonnes (plus d'un sous-en-tête de noms) ; vérifier le nombre de pages via `PROC CONTENTS`
- [x] 5.8 Écrire `corpus/gen/suppression.sas` (lignes supprimées) si 4.7 l'autorise, sinon noter la famille comme manquante
- [x] 5.9 Écrire `corpus/gen/encodage.sas` : UTF-8, et latin1/wlatin1 si 4.6 l'autorise (ou avec le SAS complet d'origine)
- [x] 5.10 Écrire le script unique d'exécution de tous les programmes de `corpus/gen/` via la commande documentée ; vérifier que chaque log est sans `ERROR`
- [x] 5.11 Vérifier, pour chaque paire de fichiers d'une famille, qu'un seul paramètre varie (revue des programmes, sans analyse d'octets)

## 6. Fichiers externes

- [x] 6.1 Identifier dans les dépôts de tests de ReadStat, Parso et pandas des fichiers couvrant big endian, 32 bits, chiffrement et autres OS ; noter source et licence avant copie
- [x] 6.2 Copier les fichiers retenus dans `corpus/external/` (données synthétiques uniquement) ; noter tout cas sans fichier disponible comme manquant, sans rien fabriquer

## 7. Manifeste

- [x] 7.1 Tenir la table de métadonnées par fichier (famille, paramètre varié, source, outil de génération)
- [x] 7.2 Écrire le script hôte qui génère `corpus/MANIFEST.md` (nom, famille, paramètre varié, source, outil, taille, SHA-256 via `shasum -a 256`) ; y ajouter les lignes explicatives pour familles ou variantes manquantes et la note sur la non-reproductibilité octet pour octet
- [x] 7.3 Vérifier que chaque fichier de `corpus/` a une ligne dans le manifeste et que les SHA-256 recalculées correspondent
- [x] 7.4 Vérifier le critère de fin : chaque famille a ses fichiers `NO` / `CHAR` / `BINARY`, ou une ligne de `MANIFEST.md` qui explique l'absence

## 8. Clôture

- [x] 8.1 Vérifier qu'aucun document d'analyse d'octets ni sortie de décompilation n'a été produit et que `src/` est inchangé
- [x] 8.2 Mettre à jour `rea_output/STATE.md` au format commun : dernière session (prompt 01, date, résultat terminé / partiel / bloqué), avancement, blocages ouverts, décisions prises (environnement retenu, commande documentée), capacités testées de `sas_small`, prochaine action (`specify-sas7bdat-read-format`, 02)

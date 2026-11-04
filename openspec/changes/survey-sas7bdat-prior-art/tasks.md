## 1. Préparation

- [x] 1.1 Relire `prompts/README.md` (conventions, modèle `STATE.md`, niveaux de confiance) et vérifier que `rea_output/` ne contient pas déjà de livrable 00 à écraser
- [x] 1.2 Créer le squelette de `rea_output/00_etat_de_l_art.md` avec les sections 1 à 5 et vérifier que les cinq titres sont présents dans l'ordre

## 2. Inventaire (section 1)

- [x] 2.1 Relever ReadStat (URL, dernière version/activité, licence lue dans `LICENSE`, date de consultation) et vérifier que l'entrée comporte les quatre champs
- [x] 2.2 Relever Parso (Java) avec les mêmes champs et vérifier l'entrée
- [x] 2.3 Relever pandas `pandas.io.sas` (URL des modules `sas7bdat` / `sas_xport`, version, licence) et vérifier l'entrée
- [x] 2.4 Relever le paquet Python `sas7bdat` (PyPI + dépôt) et vérifier l'entrée
- [x] 2.5 Relever le PDF de Matthew Shotwell et le paquet R `sas7bdat` (CRAN) avec licence vérifiée et vérifier l'entrée
- [x] 2.6 Rechercher sur crates.io toutes les crates lisant/écrivant `.sas7bdat` (dont `readstat-rs`), relever chacune et consigner la requête utilisée
- [x] 2.7 Rechercher les implémentations Go (pkg.go.dev) et JavaScript (npm) pertinentes, relever celles retenues et consigner les requêtes
- [x] 2.8 Relever la documentation SAS officielle publique (`COMPRESS=`, `ENCODING=`, formats/informats) avec URL et version de SAS documentée
- [x] 2.9 Marquer `inconnu` toute licence non trouvée, avec les emplacements consultés, et vérifier qu'aucune licence n'est citée sans référence

## 3. Matrice des capacités (section 2)

- [x] 3.1 Construire la matrice : lignes = capacités imposées par le prompt 00 (lecture, écriture, 32/64 bits, endianness, `CHAR`/`BINARY` en lecture et écriture, lignes supprimées, manquantes spéciales, labels, formats/informats, encodages, `.sas7bcat`, `.sas7bndx`, chiffrement, > 2 Go, streaming), colonnes = implémentations de la section 1
- [x] 3.2 Remplir les colonnes ReadStat et Parso avec valeur et source (fichier + ligne) par case
- [x] 3.3 Remplir les colonnes pandas, Python `sas7bdat` et Shotwell/R avec valeur et source par case
- [x] 3.4 Remplir les colonnes Rust, Go/JS et documentation SAS avec valeur et source par case
- [x] 3.5 Vérifier qu'aucune case n'est vide ni sans source (valeur `inconnu` + source consultée si non déterminable)

## 4. Points de désaccord (section 3)

- [x] 4.1 Comparer les interprétations des champs d'en-tête et de sous-en-tête entre implémentations et lister les divergences avec sources
- [x] 4.2 Relever les constantes « magiques » du chemin d'écriture de ReadStat (champ, valeur, emplacement) et vérifier que chaque entrée cite le code source
- [x] 4.3 Lister les champs ignorés par toutes les implémentations et vérifier qu'ils sont marqués `inconnu`

## 5. Liste des manques (section 4)

- [x] 5.1 Dériver les manques des sections 2 et 3 (capacités non couvertes, désaccords, constantes inexpliquées)
- [x] 5.2 Pour chaque manque, renseigner description, impact (bloquant / dégradé / confort) et méthode (analyse différentielle par défaut, décompilation justifiée sinon)
- [x] 5.3 Rattacher chaque manque à un seul prompt : 02 `specify-sas7bdat-read-format`, 03 `specify-sas7bdat-compression`, 04 `specify-sas-formats-encodings`, 05 `specify-sas7bdat-writing`, et vérifier qu'aucun manque n'est orphelin
- [x] 5.4 Signaler les manques qui nécessitent des fichiers de plateformes particulières (big endian, 32 bits, autres encodages) pour `setup-sas-oracle-corpus` (01)

## 6. Contraintes de licence (section 5)

- [x] 6.1 Définir pour chaque source la règle d'usage pendant la conception (lecture libre / documentation seule / code non lu) selon sa licence vérifiée
- [x] 6.2 Recommander une licence pour le driver avec justification (exposition R/Python, intégration Arrow Flight/Dremio) et vérifier que chaque source de l'inventaire a une règle
- [x] 6.3 Relire le document entier : français, aucune affirmation `confirmé`, aucun pseudo-code ni choix d'architecture Rust

## 7. Fichiers de suivi

- [x] 7.1 Créer `rea_output/60_inconnues.md` avec l'en-tête seul et vérifier qu'il ne contient aucune entrée
- [x] 7.2 Vérifier qu'aucun fichier hors `rea_output/00_etat_de_l_art.md`, `rea_output/STATE.md` et `rea_output/60_inconnues.md` n'a été créé ou modifié, et que `src/` n'a pas été consulté
- [x] 7.3 Créer et mettre à jour `rea_output/STATE.md` selon le modèle : prompt 00, date, résultat, état des trois critères de fin, licence recommandée dans les décisions, blocage ouvert « confirmation interne (référent licence SAS / juridique NULL) avant 02 », point d'attention émulation x86-64 pour 01, prochaine action `setup-sas-oracle-corpus`

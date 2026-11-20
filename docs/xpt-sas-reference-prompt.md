# Prompt de collecte des références SAS XPORT v5

Copier le texte ci-dessous dans une session ayant accès à une installation SAS complète.
Les résultats sont destinés à une validation locale et ne doivent pas être ajoutés au dépôt Git.

---

Tu dois produire un petit corpus de référence **SAS Transport XPORT v5** pour valider un lecteur/écrivain Rust indépendant : `UniFormat/uniformat-sas`, issue #1.

Le runtime minimal dont nous disposons démarre, mais `LIBNAME … XPORT` échoue avec `The XPORT engine cannot be found`. Cherche une installation SAS complète déjà disponible dans cet environnement. N'utilise pas pandas, ReadStat, haven ou notre propre driver comme producteur des références SAS.

## Résultat attendu

Crée un répertoire indépendant du dépôt, puis une archive `sas-xpt-v5-reference.zip`, contenant :

- les fichiers `.sas` reproductibles qui génèrent les données et les transports ;
- les `.xpt` effectivement écrits par le moteur SAS **XPORT** ;
- pour chaque membre, un schéma de référence obtenu par `PROC CONTENTS OUT=`, exporté en CSV ou JSON : nom, ordre, type, longueur, label, format et informat, avec leurs largeurs et décimales ;
- les observations de référence obtenues depuis les tables SAS sources, en CSV UTF-8 ou JSON ;
- les journaux SAS expurgés des identifiants de site et des informations de licence ;
- un `README.md` indiquant la commande exécutée, la version SAS, le système, l'encodage de session, le nombre de membres et d'observations par fichier, et les éventuelles erreurs ;
- un manifeste SHA-256 de tous les fichiers livrés.

Utilise uniquement des données synthétiques. N'inclus aucun binaire SAS, fichier de licence, donnée métier ou information personnelle. Il s'agit d'observer les résultats de SAS ; aucun désassemblage ni lecture de code propriétaire n'est nécessaire.

## Vérifier l'environnement

1. Trouve le lanceur SAS et relève la version ainsi que l'encodage de session, notamment `&SYSVLONG4`, `&SYSSCP` et `%sysfunc(getoption(encoding))`.
2. Vérifie que le moteur `XPORT` est disponible avec un programme minimal et un répertoire de sortie temporaire.
3. Si seule une installation minimale est disponible, recherche une installation complète déjà accessible. Si aucune n'est disponible, rends un diagnostic précis et les chemins vérifiés ; ne fabrique pas de fichiers de référence.

## Jeux de données à produire

Commence par une référence minimale facile à vérifier :

```sas
/* Remplacer uniquement le chemin par le répertoire de sortie choisi. */
libname refxpt xport '/chemin/de/sortie/basic-v5.xpt';
data work.BASIC;
  length VALUE 8 TEXT $8;
  label VALUE='Numeric reference';
  VALUE=1;   TEXT='one';     output;
  VALUE=-1;  TEXT='minus';   output;
  VALUE=0.5; TEXT='half';    output;
  VALUE=100; TEXT='hundred'; output;
  VALUE=.;   TEXT='missing'; output;
run;
data refxpt.BASIC;
  set work.BASIC;
run;
libname refxpt clear;
```

Puis, dans des fichiers séparés et avec des programmes complets conservés :

1. **Manquants** : une valeur présente, `.`, `._` et `.A` à `.Z`, dans cet ordre. Ajoute un identifiant numérique de ligne pour distinguer les observations.
2. **Dates et heures** : colonnes `DATE`, `DATETIME`, `TIME`, valeurs avant et après le 1er janvier 1960 et le 1er janvier 1970, fractions de seconde représentables, valeurs manquantes. Conserve les formats `DATE9.`, `DATETIME20.`, `TIME12.3` ainsi que les valeurs numériques SAS brutes.
3. **Métadonnées et limites v5** : noms de 8 caractères, label de variable et de table de 40 octets ASCII, formats/informats avec largeur et décimales, caractères de longueurs 1, 8, 12, 200 ; numériques des longueurs que SAS accepte réellement sur cette plateforme. Ne considère pas un avertissement de correction de longueur comme un succès : note la longueur réellement produite.
4. **Blancs et bourrage** : textes contenant des espaces, valeurs caractères entièrement blanches, dernière observation avec un identifiant numérique présent et un texte blanc. Ajoute un fichier avec une unique colonne caractère de longueur 12 et une dernière observation entièrement blanche ; relève combien de lignes SAS relit lui-même, car la fin du transport peut être ambiguë.
5. **Table vide** : zéro observation mais plusieurs colonnes, numériques et caractères.
6. **Bibliothèque à plusieurs membres** : deux datasets de schémas et de tailles différents dans le même XPT ; conserve leur ordre et leurs noms.
7. **Encodages** : UTF-8 avec `été` et, si une session SAS adaptée est disponible, Latin-1 avec `été` et Windows-1252 avec `€`. Les noms et formats restent ASCII. Note l'encodage effectivement utilisé, les options de session et les avertissements de transcodage ; ne présume pas que XPORT déclare son encodage dans le fichier. Si un encodage n'est pas disponible, indique-le et poursuis les autres cas.

N'utilise pas CPORT/CIMPORT : ce n'est pas XPORT. Pour cette collecte, reste en **v5** ; la validation v5 précède l'ajout de v8.

## Références indépendantes et contrôles

- Exporte schémas et observations **depuis les tables WORK sources**, avant le transport, pour ne pas valider un lecteur avec ses propres résultats.
- Pour chaque numérique, conserve une représentation SAS brute non formatée, son affichage `BEST32.` et, si possible, `HEX16.`. Les dates doivent avoir à la fois la valeur brute SAS et une représentation calendaire lisible.
- Représente explicitement les manquants par les chaînes `.`, `._`, `.A` … `.Z` dans les références ; un champ CSV vide ne permet pas de distinguer ces valeurs.
- Cite les chaînes CSV et documente comment les blancs finaux sont préservés. Ajoute une longueur et une représentation hexadécimale des octets pour les tests de blancs et d'encodage.
- Après fermeture du libref d'écriture, rouvre chaque `.xpt` avec SAS XPORT, relève son schéma et ses observations, puis compare aux tables sources. Documente toute différence, particulièrement la dernière observation blanche.
- Vérifie la présence de tous les fichiers attendus, leur taille non nulle, leur alignement sur 80 octets et le marqueur `HEADER RECORD*******LIBRARY` du transport v5.
- Un journal contenant `ERROR:` est un échec à expliquer, pas une référence validée. Tu peux livrer les autres cas réussis avec un inventaire clair.

## Réponse finale

Donne :

1. le **chemin absolu de l'archive** et son SHA-256 ;
2. la version SAS et l'encodage de chaque session utilisée ;
3. la liste des cas réussis et des cas indisponibles ;
4. les différences constatées lors de la relecture SAS ;
5. le chemin de l'installation ou la commande de lancement permettant de reproduire la collecte.

Ne prétends pas avoir exécuté SAS si ce n'est pas le cas. Une référence minimale valide vaut mieux que des fichiers fabriqués ou des erreurs masquées.

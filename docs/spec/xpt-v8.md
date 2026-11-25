# SAS Transport v8 — extensions du conteneur

Spécification du troisième lot de l'issue #1, 20 novembre 2026. Les nombres IBM,
encodages explicites, dates déterministes et transactions restent ceux de
[v5](xpt.md). L'écriture reste v5 par défaut ; `version=8` active ces extensions.

## Sources et vérification indépendante

- SAS, *Record Layout for a SAS Version 8 or 9 Data Set in SAS Transport Format*,
  octobre 2021, pages 3–11 (description des layouts uniquement) :
  <https://www.sas.com/content/dam/sasdam/documents/20260302/record-layout-of-a-sas-version-8-or-9-data-set-in-sas-transport-format.pdf>.
- ReadStat MIT : `src/sas/readstat_xport_read.c` et `readstat_xport_write.c` ;
  aucune lecture de `ieee.c`. Fichiers indépendants générés par pyreadstat 1.2.8
  pour LABELV8 et LABELV9. Les anciennes versions écrivent OBSV8 sans compteur ;
  les versions récentes lisent/écrivent un compteur décimal de 15 caractères.
- Aucun fichier produit par SAS n'est commis. Le corpus SAS v5 ne constitue pas
  une validation SAS de v8. Les fixtures commises sont synthétiques.

## Layout et limites retenues

Les marqueurs deviennent LIBV8, MEMBV8, DSCPTV8, NAMSTV8 et OBSV8. Le membre
contient son nom sur 32 octets à l'offset 8, SASDATA à 40, version à 48, OS à 56
et création à 64. Le label du **dataset reste limité à 40 octets**, sans extension.

NAMESTR reste long de 140 octets. Les offsets 88–119 contiennent le nom complet,
120–121 la longueur totale du label (u16 big-endian). Les champs historiques
contiennent des préfixes, susceptibles de couper un caractère UTF-8 : le lecteur
utilise la valeur complète avant décodage. Les noms de colonnes sont comparés
sur leur valeur complète, sans distinction de casse, jamais sur le préfixe.

L'implémentation accepte au plus 32767 variables (ordinal sur deux octets),
32 octets par nom, 256 par label de variable, 32 par nom de format/informat et
32767 par cellule caractère. Les numériques restent de largeur 2 à 8.
NAMSTV8 utilise le champ décimal de cinq positions [53,58). Toute limite de
`Limits` plus restrictive s'applique avant lecture/allocation. Les identifiants
écrits restent ASCII ; les textes utilisent l'encodage explicite choisi.

## Extensions de métadonnées

Une section facultative LABELV8 ou LABELV9 suit les NAMESTR rembourrés. Le nombre
d'entrées est dans [48,53), décimal justifié à gauche, suivi d'espaces. Chaque
entrée contient un ordinal base 1 et des longueurs u16 big-endian, puis les textes
contigus. LABELV8 décrit nom et label ; LABELV9 ajoute format et informat complets
(ex. `LONGFORMAT12.2`). Les entrées sont uniques, référencent les mêmes noms que
NAMESTR et respectent la longueur du label annoncée. LABELV9 utilise **dix octets
de préfixe pour toutes ses entrées**, même quand seule la longueur du label exige
l'extension. La section finit par des espaces jusqu'au prochain multiple de 80.

Un label long annoncé mais absent, un ordinal invalide/dupliqué, une longueur
excessive, un format incohérent ou un padding non blanc sont des erreurs. Le
budget cumulé couvre NAMESTR, extensions et métadonnées de tous les membres.
Les formats étendus sont réanalysés avant classification DATE/TIME/DATETIME.

Compatibilité haven 2.5.5 / ancien ReadStat : les en-têtes LABELV8/LABELV9 et
OBSV8 peuvent utiliser **deux champs décimaux de 15 positions**, le premier
justifié à droite contenant le compteur, le second nul. Cette variante est
acceptée en lecture ; une valeur réservée non nulle est refusée. L'écrivain
continue à produire les en-têtes canoniques décrits ci-dessus. Variante vérifiée
dans `tidyverse/haven` tag `v2.5.5`, `src/readstat/sas/readstat_xport_write.c`.

## Compteur et observations

OBSV8 peut contenir un entier décimal positif justifié à droite dans [48,63),
suivi d'espaces. Il indique le nombre exact d'observations. Le lecteur calcule
la fin par multiplication/addition vérifiées, valide le padding et exige EOF ou
un membre suivant valide. Aucun balayage des données n'est nécessaire dans ce
cas : même une signature MEMBV8/DSCPTV8 dans une cellule reste une donnée.

Un champ vide ou nul, dont l'ancien en-tête de 30 zéros, signifie compteur
inconnu : même comptage par padding et ambiguïtés que v5. Un compteur malformé,+négatif, débordant, incompatible avec la taille du fichier ou suivi de données
supplémentaires non structurées est refusé.

L'option d'écriture `row_count` permet d'annoncer un nombre exact en v8, sans
imposer Seek au flux. Chaque ligne et le commit vérifient cette promesse ; une
erreur préserve la destination. Sans cette option, l'écriture reste un flux sans
compteur, et refuse les fins blanches/signatures ambiguës comme v5. Le schéma
initial reste accessible sans lire les observations dans tous les cas.

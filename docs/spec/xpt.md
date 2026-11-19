# SAS Transport XPORT — v5

Statut : v5 implémentée et validée sur le corpus externe SAS 9.4 de l'issue #1. CPORT est exclu. La v8 est
reconnue mais refusée explicitement tant que LABELV8/LABELV9 ne sont pas validés.

## Sources et discipline

- SAS TS-140, *Record Layout of a SAS Version 5 or 6 Data Set in SAS Transport
  (XPORT) Format*, documentation publique :
  <https://support.sas.com/content/dam/SAS/support/en/technical-papers/record-layout-of-a-sas-version-5-or-6-data-set-in-sas-transport-xport-format.pdf>.
- Lecture de `pandas/io/sas/sas_xport.py` (BSD-3-Clause), et des seuls layouts de
  `ReadStat/src/sas/readstat_xport_write.c` (MIT). **Pas de lecture de `ieee.c`.**
  Le codec numérique est dérivé de la définition mathématique du format IBM.
- Les fixtures commises sont synthétiques, générées par notre code ; les tests
  de fichiers SAS externes sont ignorés par défaut. Aucun corpus SAS sous licence
  n’est commis. Voir [la validation du 19 novembre 2026](../xpt-sas-validation.md).

## Structure v5

Enregistrements de 80 octets sans fin de ligne, entiers big-endian. Les marqueurs
sont `HEADER RECORD*******`, un nom sur 8 caractères, puis
`HEADER RECORD!!!!!!!`, 30 chiffres et deux espaces.

| Enregistrement | Contenu |
|---|---|
| LIBRARY | début de bibliothèque |
| réel 1 | SAS(8), SAS(8), SASLIB(8), version(8), OS(8), blancs(24), création(16) |
| réel 2 | modification(16), blancs(64) |
| MEMBER | champs décimaux : 160 à [63,68), taille NAMESTR à [75,78) |
| DSCRPTR | début des métadonnées du membre |
| membre 1 | SAS(8), nom(8), SASDATA(8), version(8), OS(8), blancs(24), création(16) |
| membre 2 | modification(16), blancs(16), label(40), type(8) |
| NAMESTR | nombre de variables à [54,58), au plus 9999 |
| descripteurs | 140 octets chacun (135 acceptés en lecture), contigus puis padding à 80 |
| OBS | début des observations |
| données | observations contiguës, padding espace à 80 |

Le bloc MEMBER à données peut se répéter. L'ouverture du premier membre ne lit
que les métadonnées. L'énumération des membres nécessite un parcours des blocs
physiques : XPORT n'a pas d'index. Un marqueur MEMBER suivi d'un DSCRPTR à une
frontière de 80 octets est une frontière de membre ; cette signature peut aussi
apparaître dans du texte arbitraire. Il s'agit d'une ambiguïté du conteneur.

### NAMESTR

| Offset | Taille | Champ |
|---|---:|---|
| 0 | 2 | type (1 numérique, 2 caractère) |
| 2 | 2 | nhfun, réservé |
| 4 | 2 | longueur : numérique 2–8, caractère 1–200 |
| 6 | 2 | numéro (base 1) |
| 8 | 8 | nom |
| 16 | 40 | label |
| 56 | 8 | nom du format, sans largeur ni point (`DATE`, et non `DATE9`) |
| 64, 66, 68 | 2 chacun | largeur, décimales, justification |
| 70 | 2 | réservé |
| 72 | 8 | nom de l'informat |
| 80, 82 | 2 chacun | largeur et décimales de l'informat |
| 84 | 4 | position dans l'observation |
| 88 | 52 | réservé |

La longueur d'observation est `max(position + longueur)`. Les plages ne doivent
pas se chevaucher. Les colonnes, offsets, additions, métadonnées et allocations
sont bornés avant utilisation. Noms dupliqués sans distinction de casse refusés.
Les noms en écriture suivent `[A-Za-z_][A-Za-z0-9_]*`, au plus 8 octets : la casse
est préservée (les fichiers produits par les outils tiers peuvent être mixtes).

### Nombre de lignes et padding

Il n'existe **aucun compteur** v5. Pour un bloc de données de N octets et une ligne
large de L octets, on choisit le plus grand P dans 0..79 tel que les P derniers
octets soient des espaces et `(N-P) % L == 0`. Le compte est `(N-P)/L`.
Une dernière ligne entièrement blanche de moins de 80 octets peut donc être
indiscernable du padding. Le lecteur expose cette politique ; l'écrivain refuse
au commit une fin de table ambiguë plutôt que de perdre silencieusement une ligne.
Un bloc incomplet ou un résidu non blanc est une erreur, jamais ignoré.

### Nombres IBM

`(-1)^s × mantisse × 2^(4×(exposant-64)-56)`, exposant sur 7 bits, mantisse entière
sur 56 bits. Les longueurs courtes sont complétées par des zéros à droite.
Conversion vers f64 : arrondi IEEE au plus proche, égalité au pair, puis facteur
puissance de deux exact. À la borne supérieure seulement, si cet arrondi atteint
2^252, le résultat est ramené au plus grand f64 inférieur à 2^252, avec le signe
d'origine (`4FAFFFFFFFFFFFFF` / `CFAFFFFFFFFFFFFF`). Les mots IBM saturés
`7FFFFFFFFFFFFFFF` / `FFFFFFFFFFFFFFFF`, observés dans le corpus SAS 9.4 TS1M7,
se relisent ainsi comme SAS et restent réécrivables dans la plage IBM.
Écriture : normalisation hexadécimale exacte d'un f64 ;
la plage normalisée va de 2^-260 inclus à 2^252 exclu. Hors plage et infinis :
erreur. NaN et NULL numérique : `.`. Zéro négatif normalisé en zéro positif.
Les octets supprimés d'un numérique court doivent être nuls, sinon erreur.

Manquants : premier octet `.`, `_` ou `A`–`Z`, tous les autres nuls. Ils utilisent
`MissingKind`, comme sas7bdat, donc le même sidecar Arrow. Un octet `A`
suivi d'une mantisse non nulle est un nombre, pas un manquant.

### Texte et temps

UTF-8 strict par défaut, option explicite Latin1 (ISO-8859-1 véritable) ou
Windows-1252. La session SAS nommée LATIN1 du corpus écrit aussi `0x80` pour
l’euro : utiliser explicitement Windows-1252 pour reproduire ce texte. Notre
option Latin1 reste ISO-8859-1 et décode `0x80` en U+0080.
Aucun remplacement ni troncature silencieuse. La longueur est
mesurée en **octets encodés** ; les blancs finaux font partie du stockage et sont
retirés uniquement sur demande. Vide et NULL caractère deviennent des blancs.

Les dates d'en-tête restent des chaînes `ddMMMyy:hh:mm:ss` validées. La règle de
siècle est 69–99 → 1969–1999, 00–68 → 2000–2068. Écriture déterministe par défaut :
`01JAN00:00:00:00`. Aucune lecture implicite de l'horloge. DATE/TIME/DATETIME dans
les cellules gardent les conventions SAS (origine 1960), identifiées par la table
de formats existante.

## Suite de l'issue

- Adaptateur Arrow, sidecars, registre, bindings Python/R et CLI : implémentés
  dans le deuxième lot ; validation SAS v5 documentée séparément.
- V8 : confirmer sur fixtures indépendantes les extensions NAMESTR (nom long),
  LABELV8/LABELV9, les limites et la sémantique du compteur OBSV8 avant activation.

L'écrivain refuse aussi une paire de marqueurs MEMBER/DSCRPTR alignée dans les
observations, même à cheval sur deux lignes : il n'existe pas d'échappement XPORT
permettant de rendre ces octets distinguables d'une frontière de membre.

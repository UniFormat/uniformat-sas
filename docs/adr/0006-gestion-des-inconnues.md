# ADR 0006 — Gestion des inconnus de `60_inconnues.md`

- Statut : proposée (2026-10-30)
- Spécification liée : `rea_output/60_inconnues.md` (57 entrées : U01–U25, C01–C09, F01–F12, W01–W11) ; sections « cibles de décompilation » de `10` §8, `15` §8, `30` §7

## Contexte

Aucune entrée de la spécification n'est `confirmé` (pas de décompilation, confirmation NULL non consignée) ; 55 inconnues sont ouvertes. Le driver doit lire des fichiers produits par des SAS divers (9.4M1 à M7, 32 et 64 bits, big endian) et en écrire que SAS relit. Il faut donc un comportement défini pour chaque inconnu, et une règle générale pour les inconnus futurs.

## Décision

### Règle générale

- **Lecture** : `Strictness::Lenient` (défaut) = *tolérance + avertissement* : le champ inconnu est ignoré ou lu selon la règle la plus plausible de la spécification, et un avertissement typé portant l'identifiant (`U19`, `W04`…) est ajouté à `ReadReport` (jamais d'écriture sur stderr depuis le cœur). `Strictness::Strict` = tout avertissement devient une erreur `Error::Unverified(id)`. Les **contrôles de sécurité** (bornes, tailles, indices) sont des erreurs dans les deux modes. Une ligne qu'on ne sait pas décoder (flot compressé invalide) est une erreur, jamais une valeur inventée.
- **Écriture** : on n'écrit qu'une valeur `constante` ou `libre` de `20` §5 (valeur conseillée) ; toute fonction dont la spécification n'établit pas la valeur est refusée par une erreur `Unsupported(id)`. Les champs `calculé` suivent leur formule vérifiée.
- Les comportements ci-dessous sont des *défauts*; chaque ligne peut être resserrée (devenir erreur) ou relâchée par option sans changer le format.

### Table : un comportement de lecture et d'écriture pour chaque entrée

Légende : **Tol.** = tolérance silencieuse ; **Avert.** = avertissement dans `ReadReport`/`WriteReport` ; **Err.** = erreur ; **n/a** = pas de comportement (hors périmètre v1).

#### Entrées `U` (format de lecture, `10`)

| ID | Objet | Lecture | Écriture |
|---|---|---|---|
| U01 | octets 36 et 65–68 (descripteurs) | Tol. ; ne détermine ni la largeur ni l'endianness (octets 32, 35, 37) ; si 56–63 ≠ 32–39 : Avert. | `33 22 00 33 33 01 02 31` en 32–39 et copie en 56–63 ; 65–68 = `33 01 23 33` (constante) |
| U02 | `desc_50_51` / `desc_74_75` | Tol. (valeurs `03 01`, `02 01`, `00 00`) | `03 01` aux deux emplacements (constante : toute autre valeur est rejetée par SAS) |
| U03 | octet 73 | Tol. | `0x20` |
| U04 | octets 47 et 71 | Tol. | `0x14` |
| U05 | `after_page_count` | Tol. | 0 |
| U06 | formule de `file_code` | si ≠ 0 : vérifie `signature(page k) = file_code XOR (k+1)` ; écart : Avert. (page non vérifiée) ; 0 : contrôle désactivé | valeur de 32 bits aléatoire (graine configurable), signatures calculées |
| U07 | octets réservés de page | Tol. | 0 |
| U08 | `RowSize` `row_field_2` | Tol. (ignoré) | formule de `20` §5.4 (`n + 9` ; compressé `N + S + 2`) |
| U09 | `RowSize` `row_field_11/12` | Tol. | `+0x58` = 0 ; `+0x60` = `n` |
| U10 | `mix_capacity` | recalculée par le lecteur ; la valeur du fichier est ignorée | formule vérifiée (`20` §4.1) |
| U11 | `col_count_p1/p2` au-delà de 301 colonnes | Tol. ; `column_count` vient de `ColumnSize` | `p1 = n`, `p2 = 0` ; refus au-delà de la capacité de la page 0 (W04) |
| U12 | zone de références et compteurs de `RowSize` | Tol. | octets recopiés (`ROWSIZE_TAIL_WORDS` / compressé : constantes de `20` §5.4) |
| U13 | zones réservées nulles | Tol. | 0 |
| U14 | `ColumnSize` second mot | Tol. | 0 |
| U15 | `SubheaderCounts` | Tol. (ignoré) | constantes et positions de `20` §5.5 |
| U16 | `ColumnList` | Tol. (ignoré) | omis (W05) |
| U17 | `name_flags` | Tol. ; bit 0 lu comme « colonne indexée » (`Column::indexed`) | `0x0400` si nom ≤ 8 octets, sinon `0x0800` (noms de 9 à 31 octets : valeur non observée → Avert. d'écriture `U17`) |
| U18 | sous-en-tête `FFFFFFFA` de la page amd | Tol. (page `0x0400` ignorée) | n/a (index non écrits, Err. `Unsupported`) |
| U19 | bitmap de suppression hors cas observé | lecture selon la règle générale de `10` §3.5 ; Avert. `U19` si hors cas observé (page de données, multipage, 32 bits, big endian) ; Err. en `Strict` | n/a : réserve ⌈capacité/8⌉ octets sans bit à 1 ; suppression de lignes : Err. `Unsupported` |
| U20 | pages `0x4000` et combinaisons de drapeaux | les pointeurs sont lus (étape 2 de `10` §6) ; Avert. `U20` | jamais écrites |
| U21 | manquants big endian ; largeurs 1 et 2 | big endian : motif miroir de `10` §5.3, Avert. `U21` ; largeurs 1 et 2 : même règle que 3 à 7 (octets de poids fort), Avert. `U21` | non écrits (numériques de 3 à 8 seulement) ; big endian : Err. `Unsupported` |
| U22 | fiabilité du code d'encodage | `encoding_override` prioritaire ; sinon code de l'en-tête ; si 29/62 et octets UTF-8 valides non ASCII : Avert. `U22` suggérant l'override | code exact de l'encodage écrit |
| U23 | règle de la taille de page | accepte `P` quelconque entre 256 (observé) et `Limits::max_page_length` (16 Mio par défaut), multiple de 8 ; sinon Err. | défaut 65 536 ; agrandie (multiple de 8, ≤ 1 Mio) si les métadonnées ou une ligne ne tiennent pas ; `HL = P` |
| U24 | offsets 32 bits de `FormatAndLabel` | lecture 32 bits supportée, Avert. `U24` (un seul fichier de référence) | n/a (32 bits non écrit) |
| U25 | contrôles à l'ouverture de SAS | contrôles propres au driver : magic, tailles, `page_count × P ≤ len`, bornes ; pas de reproduction des rejets de SAS | écriture des `constante` et `calculé` de `20` §5 (les rejets observés par mutation sont la liste des contrôles de SAS) |

#### Entrées `C` (compression, `15`)

| ID | Objet | Lecture | Écriture |
|---|---|---|---|
| C01 | modules `sasyzcrl`/`sasyzcr2` | n/a (aucun comportement) | n/a |
| C02 | critère de désactivation de la compression | n/a | `Compression::Auto` : pas de compression sous 21 octets par ligne (observé) ou si le gain estimé est nul ; `Char`/`Binary` explicites : refusé sous 16 octets (écrit sans compression + Avert.) |
| C03 | seuils et choix des commandes RLE | toutes les commandes de `15` §3 sont décodées (dont `0x50`, `0x90`, `0xA0`, `0xB0`) | le cœur n'émet que les commandes dont la spécification établit la forme (littéraux `0x80`/`0x00`, suites `0xC0`–`0xF0`, `0x40`/`0x60`/`0x70`) ; flot toujours décodable |
| C04 | algorithme de recherche des références RDC | décodage de toutes les commandes | recherche de références propre au driver (décalage ≥ 3, fenêtre ≤ 4 098) ; flot valide, pas identique à SAS |
| C05 | ligne brute ou compressée (RDC) | accepte les deux formes (ligne brute : `compression` sans bit 2, `length = row_length`) | brute si la ligne fait ≥ 16 octets, le flot n'est pas plus court et le début n'est pas une signature de sous-en-tête ; sinon flot compressé (`20` §4.2) |
| C06 | commandes jamais observées | décodées selon les formules ; Avert. `C06` à la première occurrence | non émises |
| C07 | page `0x9000` : bloc final et octets `BD 7F` | page ignorée (non lue) | marqueur écrit selon `20` §4.2 point 7 ; `P ≥ 65 536` ; octets 38–39 à 0 |
| C08 | flot invalide et cas limites | Err. `CorruptRow { page, row }` ; option `skip_corrupt_rows` : ligne nulle + Avert. `C08` | n/a ; le décompresseur du driver est testé par fuzzing (`docs/architecture.md` §6.4) |
| C09 | octets de compression autres que 0, 1, 4 | bit 2 → ligne compressée ; autre valeur : ligne brute si `length = row_length`, sinon ignorée ; **bit 3 → ligne lue comme supprimée** (corrélé à `deleted_count` sur 4 fichiers, jalon J2) avec Avert. `C09` (erreur en mode strict) | n'écrit que 0, 1, 4 |

#### Entrées `F` (formats, encodages, `30`)

| ID | Objet | Lecture | Écriture |
|---|---|---|---|
| F01 | inventaire exhaustif des formats natifs | table de classification embarquée (241 noms) ; nom inconnu = format utilisateur, type de stockage conservé, métadonnée `sas.format` recopiée | nom recopié tel quel |
| F02 | repli en notation E | n/a v1 (affichage non requis) | n/a |
| F03 | très petites valeurs, chiffres de `BEST` | n/a v1 | n/a |
| F04 | formats absents de l'oracle | classés d'après la documentation (table embarquée) ; aucune vérification | n/a |
| F05 | dates hors plage, anomalies de calendrier | `OutOfRange` (ADR 0003) : `Null` + compteur `out_of_range` par défaut | date hors 1582-01-01…20 000 : Err. (`Unrepresentable::Error`) |
| F06 | heures très grandes, décimales de `HHMM`/`MMSS` | `OutOfRange` pour `Time64` ; `TimeMode::Duration` en option | n/a |
| F07 | catalogue : reste des entrées du répertoire | n/a v1 : `.sas7bcat` non lu | n/a |
| F08 | catalogue : plages, `OTHER`, `PICTURE`, options | n/a v1 | n/a |
| F09 | `FORMATC`, `INFMTC`, plusieurs catalogues, pages d'extension | n/a v1 | n/a |
| F10 | désaccords sur les codes d'encodage | codes vérifiés (0, 20, 29, 40, 62, 138) sans avertissement ; autres codes de la table : Avert. `F10` ; code inconnu : Err. `UnknownEncoding(code)` sauf `encoding_override` | 20, 29 (déconseillé : pyreadstat 1.2.9 le refuse), 62 |
| F11 | transcodage sur octets invalides hors UTF-8 | `InvalidUtf8` (ADR 0003), défaut `Replace` + compteur | caractère non encodable : `Unrepresentable` (ADR 0005) |
| F12 | noms SAS des encodages, codes 0 et 204 | codes 0/204 : détection UTF-8 puis `windows-1252`, Avert. `F12` | codes 0 et 204 jamais écrits |

#### Entrées `W` (écriture, `20`)

| ID | Objet | Lecture | Écriture |
|---|---|---|---|
| W11 | tables compressées de plus de 65 535 lignes — résolue (2026-11-02) | sans objet (le cumul de la page marqueur n'est pas utilisé en lecture) | aucune limite |
| W02 | marqueur : ≥ 3 pages de lignes, petites pages, remplissage | tolérance (marqueur ignoré) | forme observée généralisée (première et dernière page de lignes) + Avert. `W02` à partir de trois pages de lignes ; compression refusée si `P < 65 536` |
| W03 | pages `0x4000` et `0x0400` | pointeurs lus, Avert. `W03` | jamais écrites |
| W04 | métadonnées sur plusieurs pages | lecture multi-pages supportée (`10` §3.2) | Err. `TooManyColumns` au-delà de la capacité de la page 0 (549 colonnes numériques pour `P = 65 536`) |
| W05 | `ColumnList` omis | Tol. | omis |
| W06 | portée des champs `libre` | Tol. | écrire la valeur conseillée ; ne jamais exposer de réglage de ces champs hors `WriteOptions::creator` |
| W07 | `RowSize+0x58` | Tol. | 0 |
| W08 | fichiers de plus de 2 Go | tous les offsets, tailles et compteurs en `u64` ; Avert. `W08` au-delà de 2 Gio (non testé) | idem + Avert. `untested_large_file` |
| W09 | release et hôte | Tol. | `9.0401M7`, `Linux` (surchargeables) |

### Nouveaux inconnus

Tout champ rencontré hors des cas de la spécification (signature de sous-en-tête non listée, type de page inconnu, valeur d'énumération inconnue) : Avert. `UNKNOWN-n` avec les octets hexadécimaux concernés dans `ReadReport` (jamais masqué) ; en mode `Strict` : Err. Le test de non-régression du corpus garantit que les 232 fichiers analysés par `10` §9 (173 non compressés au jalon 1, les autres au jalon 2) se lisent sans erreur en `Lenient` et sans avertissement hors liste attendue.

## Options étudiées

| Option | Pour | Contre |
|---|---|---|
| A. Tout en erreur (strict) | aucun comportement douteux | rejette des fichiers réels (9.4M1, OUTREP, anciens) que les autres lecteurs lisent |
| B. Tout toléré en silence | simple | masque des divergences ; contraire à la règle « on ne masque jamais un inconnu » (`prompts/README.md`) |
| C. Tolérance + avertissement typé, mode strict optionnel (retenue) | lit les fichiers réels, trace chaque inconnu avec son identifiant | surface d'API (`ReadReport`) à maintenir |

## Conséquences

- (+) Chaque entrée de `60` figure ci-dessus avec un comportement de lecture et d'écriture (55 entrées : 25 U, 9 C, 12 F, 9 W).
- (+) Quand une inconnue est levée (SAS complet, décompilation), la ligne correspondante change de comportement sans toucher à l'architecture.
- (−) Les « n/a v1 » de F02–F09 reportent l'affichage des formats et la lecture des catalogues à des jalons optionnels : un utilisateur qui veut restituer `DATE9.` en texte doit le faire lui-même à partir du type Arrow.
- (−) Le mode `Lenient` peut produire des données plausibles mais non vérifiées pour des cas non observés (U19, U21, U24) : l'avertissement est la seule garantie.

# Validation XPORT v8 — 20 novembre 2026

Le troisième lot de l'issue #1 ajoute la lecture automatique v5/v8 et l'écriture
v8 explicite après fusion de #3. La [spécification](spec/xpt-v8.md) et
[l'ADR 0010](adr/0010-xport-v8.md) décrivent les choix et limites.

## Vérifications locales

- Suite Rust : 123 tests réussis, 30 tests optionnels ignorés.
- Python 3.9 / PyArrow 21 : 26 tests réussis, 13 optionnels ignorés. Le corpus
  externe SAS v5 est activé : ses 14 fichiers, 15 membres et 116 observations
  sont revalidés, y compris les allers-retours et les 28 codes de manquants.
- Clippy workspace/toutes cibles sans avertissement ; formatage, frontières de
  dépendances, tables générées et compilation du harnais de fuzz vérifiés.
- Tests de conversion via le registre partagé : XPT v8 → sas7bdat → XPT v8,
  noms longs et cellules de 250 octets conservés. La rétrogradation v5 implicite
  échoue avant création du fichier.

Les cas obligatoires couvrent LABELV8, LABELV9 (y compris une entrée n'ayant qu'un
label long au milieu d'une section v9), préfixes de noms identiques, UTF-8 coupé
dans le préfixe historique, formats longs, 10 000 colonnes, limites de noms/labels/
cellules, lectures des seules métadonnées, compteurs hérités/nuls/positifs,
membres multiples, troncatures, budgets et discordances de compteur atomiques.
Un compteur positif préserve les lignes finales blanches et les signatures de
membre littérales dans les cellules sans scanner le bloc d'observations.

## Interopérabilité indépendante

pyreadstat 1.2.8 produit des références synthétiques LABELV8 et LABELV9, relues par
le binding Python ; il relit aussi nos sorties et leurs noms/labels/formats longs.
Les tests `bindings/python/tests/test_xpt_v8.py` se reproduisent avec pytest quand
pyreadstat et pandas sont disponibles. Le paquet R ajoute les mêmes échanges
dans `bindings/r/rpkg/tests/testthat/test-xpt.R` avec haven ; l'exécution R utilise
la CI, car R n'est pas installé sur ce poste.

Certaines versions anciennes de l'écrivain ReadStat mélangent des préfixes de
6 et 10 octets dans une même section LABELV9. Les références indépendantes évitent
ce défaut du producteur ; notre sortie mixte utilise uniformément 10 octets et
est relue par ReadStat. Le lecteur refuse les entrées incohérentes.

Les trois fixtures `tests/data/xpt/v8-*.xpt` proviennent exclusivement du générateur
Rust du dépôt ; leur procédure de régénération est dans le README de ce dossier.

## Limites des preuves

Aucune nouvelle exécution SAS `%LOC2XPT`/`%XPT2LOC` v8 n'a été effectuée. Le
corpus SAS fourni auparavant est exclusivement v5. Les tests croisés v8 ne
constituent pas une certification SAS ou réglementaire ; aucun fichier SAS sous
licence n'est commis. Le harnais de fuzz est compilé, sans campagne prolongée.

V5 reste le défaut d'écriture. Les limites v8 sont mesurées en octets encodés,
et le label du dataset reste limité à 40 octets. Pour un flux sans nombre de
lignes connu, les ambiguïtés de fin de table continuent d'entraîner une erreur.

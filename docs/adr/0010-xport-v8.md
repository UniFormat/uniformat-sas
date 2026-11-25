# ADR 0010 — extensions v8 sur le codec XPORT existant

La v5 étant validée puis fusionnée (#3), le troisième lot active la lecture
automatique v5/v8 et l'écriture v8 explicite. La [spécification](../spec/xpt-v8.md)
précède l'implémentation ; les couches et dépendances de l'ADR 0008 sont conservées.

Le codec reste sans I/O. Il valide les en-têtes, les extensions de labels et les
formats ; la couche I/O lit leurs tailles sous budget avant allocation. Une
version portée par chaque membre interdit les mélanges de marqueurs incohérents.
Les adaptateurs Arrow, Python, R et CLI partagent les mêmes options et métadonnées.

Le compteur OBSV8 positif devient une frontière exacte ; les fichiers historiques
sans compteur gardent la règle v5. Un `row_count` explicite permet un flux compté
sans Seek ni collecte préalable ; toute discordance empoisonne la transaction.
On ne devine pas le compteur à partir d'une métadonnée Arrow qui pourrait être
périmée après filtrage. V5 reste le choix d'écriture par défaut.

Les tests couvrent les frontières, erreurs hostiles, métadonnées seules, les deux
formes LABEL, les fins blanches, les flux comptés et l'interopérabilité ReadStat/
haven. Une relecture dans une véritable session SAS v8 reste une preuve distincte
de ces tests et de la validation externe SAS v5 déjà effectuée.

# ADR 0009 — adaptateurs XPORT et conversions communes

Les valeurs SAS deviennent Arrow dans les mêmes constructeurs de colonnes pour
sas7bdat et XPORT. L'accès aux cellules reste spécifique au codec : IBM pour XPT,
IEEE pour sas7bdat. Aucune fausse page sas7bdat n'est fabriquée pour réutiliser du
code. Le schéma partage le mapping des colonnes et les métadonnées `sas.*` ;
les informations propres au conteneur utilisent `xpt.*`.

L'écriture partage le plan des champs Arrow et les conversions de cellules,
puis appelle l'écrivain transactionnel du format. XPORT reste strict : pas de
politique implicite de remplacement, pas d'horloge système, et tout échec (y
compris une conversion Arrow) empêche le commit. Les codes sidecar hors 0..28
sont des erreurs.

Les flux sont bornés en lignes, colonnes et octets. L'inférence des largeurs
caractère requiert les données : l'API de commodité retient au plus un budget
explicite ; l'écrivain de lots reste en flux lorsque les largeurs sont connues.
L'index multi-membres est calculé explicitement et le choix se fait par nom ou
index. Le premier schéma reste accessible sans cette indexation.

Python utilise l'adaptateur Arrow. R utilise directement `uniformat-sas-io`,
conformément au graphe de dépendances existant ; les NA étiquetés de haven
préservent les manquants spéciaux. Le registre commun expose `xpt` à la CLI.

La restitution temporelle native utilise la microseconde. À l'écriture XPT,
les Date64 qui ne représentent pas un jour entier, les fractions plus fines que
la microseconde et les valeurs qui perdent leur précision lors du passage par
les secondes SAS sont refusées. `dates=float` permet de relire les valeurs SAS
brutes sans conversion temporelle. Les booléens et entiers se relisent comme
nombres SAS ; leur type Arrow initial n'est pas encodé dans XPORT.

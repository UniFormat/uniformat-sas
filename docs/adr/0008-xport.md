# ADR 0008 — démarrer XPORT par un codec v5 indépendant des I/O

Décision : `uniformat_sas::xpt` traite uniquement des buffers ;
`uniformat_sas_io::XptFile` pilote les plages et `XptWriter` réutilise `AtomicFile`.
On partage les colonnes, formats, valeurs manquantes et cellules avec sas7bdat,
mais jamais son parseur de pages ou sa représentation IEEE sur disque.

Les choix de padding, UTF-8 strict, noms sensibles à la casse avec unicité
insensible à la casse, dates reproductibles et plage IBM sont spécifiés dans
[../spec/xpt.md](../spec/xpt.md). La perte d'information est une erreur en écriture.

L'ouverture du premier schéma ne parcourt pas les données. L'index des membres
et le compte de lignes sont calculés explicitement, car le format ne les stocke
pas ; ils ne doivent pas être annoncés comme gratuits ou exacts pour des fichiers
ambigus. Les observations sont ensuite lues une par une avec une mémoire bornée.

Conséquences : le premier lot est utilisable en Rust sans dépendance Arrow,
conforme à la liste fermée des dépendances. Les adaptateurs et la v8 restent des
jalons séparés de #1 ; une détection LIBV8 ne vaut pas une prise en charge v8.

## Validation des décisions

Le 18 novembre 2026, l'utilisateur a confirmé les sept décisions : v5 avant v8,
UTF-8 par défaut avec Latin-1 et Windows-1252 explicites, NaN en manquant SAS,
dates reproductibles, refus des fins ambiguës, transactions et ouverture du
schéma sans données. La suite de #1 applique ces choix aux adaptateurs.

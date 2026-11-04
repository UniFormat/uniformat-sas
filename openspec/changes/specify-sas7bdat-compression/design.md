## Context

Voir `proposal.md` (Why). Ce changement s'appuie sur les sorties de `specify-sas7bdat-read-format` (`rea_output/10_format_sas7bdat.md`, `rea_output/tools/ref_reader.py`) et sur l'oracle et le corpus de `setup-sas-oracle-corpus` (`corpus/`, `corpus/MANIFEST.md`, variantes `NO` / `CHAR` / `BINARY`). `src/` (sas_small, SAS 9.4 TS1M7, ELF x86-64 Linux, UTF-8) contient les modules `sasexe/sasyzcrl` et `sasexe/sasyzcr2` ; il est en lecture seule. L'hôte est macOS : SAS ne tourne que dans un Linux x86-64 (conteneur, VM ou machine distante) mis en place par le prompt 01. Exigences : `specs/sas7bdat-compression/spec.md`.

## Goals / Non-Goals

**Goals:**
- Une spec de compression suffisante pour écrire un décompresseur et un compresseur sans lire le code SAS.
- Distinguer clairement ce qu'un écrivain doit respecter de ce qu'il peut choisir.
- Des outils Python de référence et des vecteurs réutilisables comme tests par le driver Rust.

**Non-Goals:**
- Reproduire octet pour octet les sorties compressées de SAS.
- Écrire un `.sas7bdat` complet, ni faire relire par SAS les sorties de `ref_compress.py` (reporté à `specify-sas7bdat-writing`).
- Optimiser les performances des outils Python.

## Decisions

1. **Méthode différentielle avant décompilation.** On compare d'abord les fichiers `NO` / `CHAR` / `BINARY` d'un même jeu de données pour déduire la détection et les opcodes ; `sasyzcrl` / `sasyzcr2` ne sont décompilés que pour les opcodes et règles que les fichiers n'expliquent pas. *Alternative* : décompiler d'emblée les deux modules — rejetée car plus coûteuse, plus exposée juridiquement (l'exception d'interopérabilité ne couvre que les informations non disponibles autrement) et produisant des preuves moins lisibles.
2. **Hypothèses publiques = points de départ.** ReadStat, Parso, pandas et le PDF de Shotwell orientent la recherche (RLE pour `CHAR`, RDC pour `BINARY`) mais ne valent au mieux que `inféré`. *Alternative* : les adopter telles quelles — rejetée, elles peuvent être incomplètes sur les opcodes rares ou les règles d'écriture.
3. **Décompression d'abord par les fichiers, compression d'abord par le binaire.** Les règles de décision (seuils, fenêtre RDC, choix brut / compressé) ne se lisent pas de façon fiable dans les fichiers : c'est la cible légitime de la décompilation du chemin d'écriture. Le critère d'écriture est la décodabilité par SAS, pas l'identité des octets.
4. **Oracle SAS dans Linux x86-64.** Les vecteurs `decompress(SAS(x))` sont produits par `sas_small` dans l'environnement du prompt 01 (conteneur ou VM), jamais par contournement de licence (`elssrv`, SETINIT). *Alternative* : émulation Rosetta/qemu ad hoc sur le Mac — non retenue hors de l'environnement déjà validé par 01.
5. **Séparation analyse / conception.** Les preuves brutes vont dans `rea_output/evidence/` ; `rea_output/15_compression.md` décrit les algorithmes en tables et pseudo-code anglais, sans recopier de code décompilé, pour que `design-rust-driver` puisse ne lire que la spec.
6. **Outils écrits depuis la spec.** Le décompresseur de `ref_reader.py` et `ref_compress.py` sont écrits à partir de `15_compression.md` seulement ; leurs échecs sur le corpus signalent un trou de spec, corrigé dans la spec avant le code.

## Risks / Trade-offs

- [Licence SAS / interopérabilité : la décompilation de modules SAS sous licence Apache doit être couverte] → tâche initiale consignant dans `rea_output/STATE.md` la confirmation interne (référent licence SAS / juridique NULL) que l'analyse à des fins d'interopérabilité est couverte ; décompilation limitée aux manques non documentés publiquement ni déductibles du corpus.
- [Émulation x86-64 sur Mac lente ou instable pour l'oracle] → réutiliser l'environnement validé par `setup-sas-oracle-corpus` ; produire les vecteurs SAS par lots et les versionner dans `rea_output/tools/compression_vectors/`.
- [Hypothèses publiques erronées ou incomplètes (opcodes rares, longueurs limites)] → cas limites dédiés dans les vecteurs ; tout écart consigné dans `60_inconnues.md`.
- [Corpus insuffisant pour exercer tous les opcodes] → générer via l'oracle des jeux ciblés (séquences longues, octets répétés, données aléatoires) en complément du corpus 01.
- [Règle classée « libre » à tort, rendant une sortie illisible par SAS] → classement justifié par preuve ; vérification SAS effective reportée et tracée pour `specify-sas7bdat-writing`.
- [Contamination de la conception par du code SAS] → séparation stricte `evidence/` / spec (décision 5).

## Open Questions

- Granularité exacte de la compression (ligne seule ou autres structures) : réponse attendue de l'étape « Où et quoi », sans impact sur le découpage des tâches.

# D-006 — Lire les `case.toml` avec un mini-parseur écrit à la main

- **Identifiant :** D-006
- **Date :** 2026-11-21
- **Auteurs :** Chaimae Faris
- **Statut :** adoptée
- **Commits concernés :** `116ffe3` (proposition), `b81230a` (crate `corpus`)

## 1. Contexte

Chaque cas du corpus décrit ses exécutions dans un `case.toml` (`corpus/FORMAT.md`). La bibliothèque `toml` n'est pas
dans la liste des dépendances autorisées (README §3), et les générateurs de parseurs (`pest`, `nom`…) sont interdits.
Les cas cachés seront donnés le jour de la soutenance, au même format : leur lecteur doit déjà marcher ce jour-là.

## 2. Options envisagées

### Option A — demander la validation de `toml`
Compromis : un parseur complet et éprouvé. Mais il faut l'accord de l'enseignant, le délai est incertain, et c'est
une dépendance de plus à justifier.

### Option B — un mini-parseur écrit à la main, limité au sous-ensemble utilisé
Clés nues, chaînes `"…"` avec échappements, entiers signés, booléens, tables en ligne `{ … }`, tableaux `[ … ]`
(éventuellement sur plusieurs lignes), en-têtes `[[exec]]` et commentaires `#`.

Compromis : environ 200 lignes à écrire et à tester nous-mêmes, mais aucune dépendance, et un premier parseur à la
main qui servira au bonus B3. En contrepartie, une construction TOML hors du sous-ensemble ne sera pas lue.

### Option C — convertir le corpus en JSON une fois pour toutes
Compromis : rapide, mais les cas cachés arriveront en TOML, et il faudrait les convertir à la main le jour même.

## 3. Ce que l'assistant IA a proposé

L'option B. Le sous-ensemble a été relevé sur les 65 cas publics : `description`, `category` et `clause` (chaînes),
`[[exec]]`, `budget` (entier), `mem = { 3 = 42 }`, `answers = [{ sense = 0 }, { act = true }]` et `load_error`. On y
ajoute `rand`, `sense_fault`, les entiers négatifs et les tableaux sur plusieurs lignes, que `FORMAT.md` documente
mais qu'aucun cas public n'utilise. Tout le reste de TOML (dates, flottants, tables `[x]`, clés pointées) est refusé
avec un numéro de ligne.

Le parseur est une descente récursive sur un curseur de caractères qui compte les lignes. Les tables gardent l'ordre
du fichier (`Vec<(String, Value)>`), ce qui suffit à des cas de quelques lignes.

L'assistant a aussi écrit le code de la tâche T08 (`crates/corpus`) pour faire passer les 18 tests du plan, et il y a
ajouté une règle que le plan ne donnait pas : une clé inconnue dans `case.toml`, ou un champ inconnu dans
`expected.json`, est refusé au lieu d'être ignoré. La VM a passé les 65 cas publics au premier essai, sans correction.

Ce que nous en avons vérifié : `cargo test -p corpus` passe (18 tests), dont `the_public_corpus_passes_in_process`, 65
cas sur 65. Nous avons relu `crates/corpus/src/toml.rs`, en particulier la façon dont le curseur compte les lignes
(`bump`). Nous avons regardé plusieurs `case.toml` du corpus : on n'y trouve que des chaînes, des entiers, des
booléens, des tables `{ … }`, des tableaux `[ … ]` et des en-têtes `[[exec]]`. Le test `errors_carry_a_line_number`
montre qu'un flottant, une table `[x]` ou une chaîne non terminée sont refusés avec leur numéro de ligne.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons l'option B : notre propre lecteur, limité au sous-ensemble du corpus.

La bibliothèque `toml` n'est pas dans la liste autorisée, et attendre l'accord de l'enseignant aurait bloqué T08 sans
savoir pour combien de temps. Convertir les cas en JSON ne règle rien non plus : les cas cachés arriveront en TOML le
jour de la soutenance, et les convertir à la main ce jour-là serait trop risqué.

Le sous-ensemble est petit : `toml.rs` fait environ 280 lignes, et chaque règle a son test. C'est aussi un premier
parseur écrit à la main, qui nous servira pour le langage du bonus B3.

Nous gardons la règle ajoutée par l'assistant : une clé inconnue est refusée. Un `budjet = 12` mal écrit donne une
erreur claire, au lieu de lancer le cas en silence avec le budget par défaut de 1000.

## 5. Ce qui nous ferait changer d'avis

Un cas caché qui utilise une construction TOML hors du sous-ensemble. Le test `errors_carry_a_line_number` garantit
qu'on le saura par un message clair qui donne la ligne, pas par un résultat faux.

## Historique

- 2026-11-21 — proposée.
- 2026-11-21 — adoptée.

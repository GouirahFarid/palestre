# D-006 — Lire les `case.toml` avec un mini-parseur écrit à la main

- **Identifiant :** D-006
- **Date :** 2026-11-21
- **Auteurs :** Chaimae Faris
- **Statut :** proposée
- **Commits concernés :** —

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

Ce que nous en avons vérifié : *à compléter.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

Un cas caché qui utilise une construction TOML hors du sous-ensemble. Le test `errors_carry_a_line_number` garantit
qu'on le saura par un message clair qui donne la ligne, pas par un résultat faux.

## Historique

- 2026-11-21 — proposée.

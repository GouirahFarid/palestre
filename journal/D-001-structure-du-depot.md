# D-001 — Structure du dépôt : un workspace de petites crates

- **Identifiant :** D-001
- **Date :** 2026-10-26
- **Auteurs :** Farid GOUIRAH
- **Statut :** proposée
- **Commits concernés :** —

## 1. Contexte

Le sujet (README §5, critère 1) demande une séparation nette entre le format PBC (`pbc`), le moteur d'exécution et
la couche réseau (`exec`). Plusieurs personnes travaillent en parallèle sur ces composants. `serde` ne doit jamais
entrer dans la boucle interne de la VM (README §3), et `unsafe` est interdit dans tout le moteur d'exécution et de
décodage.

## 2. Options envisagées

### Option A — une seule crate avec des modules
`src/pbc`, `src/vm`, `src/exec` dans une crate unique.

Compromis : un seul `Cargo.toml` et aucune cérémonie. En revanche, le compilateur ne vérifie aucune frontière : rien
n'empêche `vm` d'importer `serde_json`, et toutes les dépendances sont visibles partout.

### Option B — un workspace multi-crates
Une crate par composant, réunies dans un workspace Cargo.

Compromis : chaque crate déclare ses propres dépendances, si bien que `vm` ne *peut* pas utiliser `serde`. Les lints
et `forbid(unsafe_code)` se règlent par crate, et deux personnes qui travaillent sur deux crates touchent rarement les
mêmes fichiers. En contrepartie : un `Cargo.toml` par crate, et des chemins `pbc::…` à écrire entre crates.

## 3. Ce que l'assistant IA a proposé

L'option B, avec sept crates : `pbc` (format), `vm` (machine), `exec` (protocole de l'annexe K), `asm` (assembleur),
`corpus` (rejeu des cas de test), `arena` (monde de l'annexe E) et `palestre` (binaire CLI). Les lints communs
(`unsafe_code = "forbid"`, `unwrap_used`, `expect_used`, `panic` en `deny`) vont dans `[workspace.lints]`.
`clippy::indexing_slicing` et `clippy::arithmetic_side_effects` sont en `deny` dans `pbc` et `vm` seulement, pour
qu'aucun accès indexé ni calcul ne puisse y paniquer.

Ce que nous en avons vérifié : *à compléter par l'équipe.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger par l'équipe.*

## 5. Ce qui nous ferait changer d'avis

Un type qui migre de crate chaque semaine (signe que les frontières sont mal tracées), ou un temps de compilation
incrémentale qui gêne le live-coding en soutenance. On fusionnerait alors `asm` dans `pbc`, ou `corpus` dans
`palestre`.

## Historique

- 2026-10-26 — proposée.

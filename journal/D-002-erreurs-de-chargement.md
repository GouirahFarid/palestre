# D-002 — Les erreurs de chargement sont distinctes des fautes d'exécution

- **Identifiant :** D-002
- **Date :** 2026-10-26
- **Auteurs :** Farid GOUIRAH
- **Statut :** proposée
- **Commits concernés :** `890c92f` (proposition), `d94a247` (chargeur)

## 1. Contexte

L'annexe B.5 range `BadHeader` parmi les seize fautes, mais précise qu'elle est levée *avant* l'exécution et qu'elle
« n'apparaît jamais dans les fautes d'un tour en cours ». Le sujet (README §5) évalue la capacité à rendre les états
invalides irreprésentables.

## 2. Options envisagées

### Option A — un seul `enum Fault` à seize variantes
Compromis : fidèle à la liste de B.5, un seul type à connaître. En revanche, rien n'empêche la VM de rendre
`BadHeader` en cours de tour : le compilateur ne protège pas contre ce mélange.

### Option B — deux types : `LoadError` pour le chargement, `Fault` (quinze variantes) pour l'exécution
Compromis : une faute de chargement en cours de tour devient impossible à écrire. `LoadError` peut aussi détailler la
cause (magique, version, taille…) pour le débogage, tout en se nommant `BadHeader` sur le fil. En contrepartie, il faut
une conversion explicite vers la chaîne normative.

## 3. Ce que l'assistant IA a proposé

L'option B. `LoadError` détaille la cause et `fault_name()` rend toujours `"BadHeader"`. Effet de bord utile :
`Fault::from_name("BadHeader")` rendra `None`, ce qui fera refuser naturellement un `SENSED` qui nomme une faute de
chargement (annexe K.3).

L'assistant a aussi écrit le code du chargeur (`crates/pbc/src/program.rs`, commit `d94a247`) pour faire passer les
dix tests de `crates/pbc/tests/load.rs`, eux aussi tirés de son plan. Il lit le fichier avec un curseur (`Reader`)
qui ne passe que par `get`, si bien qu'aucune lecture ne peut sortir du tampon, et il vérifie chaque borne de A.1
avant de lire ou d'allouer ce qu'elle protège.

Ce que nous en avons vérifié : *à compléter par l'équipe.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger par l'équipe.*

## 5. Ce qui nous ferait changer d'avis

Une version 2 de la spécification qui ferait lever `BadHeader` en cours d'exécution, par exemple un chargement
dynamique de code.

## Historique

- 2026-10-26 — proposée.

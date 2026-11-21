# D-002 — Les erreurs de chargement sont distinctes des fautes d'exécution

- **Identifiant :** D-002
- **Date :** 2026-10-26
- **Auteurs :** Farid GOUIRAH
- **Statut :** adoptée
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

Ce que nous en avons vérifié : `cargo test -p pbc` passe, dont les dix tests de `crates/pbc/tests/load.rs`. Nous avons
relu `crates/pbc/src/program.rs` et son curseur `Reader`. Les trois cas `header` du corpus public (001, 002 et 003)
sont refusés et le cas 040 se charge (test `public_corpus_header_cases_are_refused`). Un fichier qui annonce plus de
4095 constantes est refusé avant que la table soit lue ou allouée (test
`constant_count_is_bounded_before_reading_the_table`).

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons l'option B : deux types, `LoadError` pour le chargement et `Fault` (quinze variantes) pour l'exécution.

Avec un seul type, rien n'empêcherait la VM de rendre `BadHeader` au milieu d'un tour, alors que B.5 dit que cela
n'arrive jamais. Avec deux types, cette erreur ne peut même pas s'écrire : `Machine::new` ne reçoit qu'un `Program`
déjà chargé, et les fautes de la machine sont des `Fault`.

Le deuxième avantage nous sert au débogage : `LoadError` dit exactement ce qui ne va pas (magique, version, taille…),
alors que sur le fil tout s'appelle `BadHeader`, grâce à `fault_name()`.

Enfin, comme `Fault::from_name("BadHeader")` rend `None`, le serveur refusera sans code supplémentaire un `SENSED` qui
nommerait une faute de chargement (K.3).

## 5. Ce qui nous ferait changer d'avis

Une version 2 de la spécification qui ferait lever `BadHeader` en cours d'exécution, par exemple un chargement
dynamique de code.

## Historique

- 2026-10-26 — proposée.
- 2026-11-21 — adoptée.

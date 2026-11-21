# D-005 — L'ordre des vérifications dans une instruction

- **Identifiant :** D-005
- **Date :** 2026-10-27
- **Auteurs :** Chaimae Faris
- **Statut :** adoptée
- **Commits concernés :** `974c7aa` (proposition), `8285cf0` (`CONST`), `7d34639` (`CALL`), `856d8a0` (`SENSE`, `ACT`, `RAND`)

## 1. Contexte

Plusieurs fautes peuvent s'appliquer à une même instruction. La spec fixe une partie de l'ordre : B.2 impose
décodage, puis gaz, puis exécution, et le corpus public montre que `StackUnderflow` passe avant toute autre
vérification (cas 046b, 047b, 054b, 063) et que `AlreadyActed` est levée sans rappel (cas 032). Pour le reste, la spec
est muette : `CONST k` hors de la table sur une pile pleine ? `CALL` avec une pile d'appels pleine et une cible
invalide ? `SENSE` sur une pile pleine, rappel posé ou non ?

## 2. Options envisagées

### Option A — « tout vérifier, puis agir »
Arité, validité des opérandes, place sur la pile, puis seulement le rappel.

Compromis : aucun rappel n'est posé si la réponse ne pourra pas être empilée. Mais c'est un ordre que le texte de B.3
ne décrit nulle part.

### Option B — « dépiler, calculer, empiler »
Lecture littérale de la notation `a b -- r` : l'arité d'abord, puis les étapes propres à l'instruction dans l'ordre du
texte de B.3 (lire la table, poser le rappel, empiler le cadre puis sauter), et enfin l'empilement du résultat, qui
peut lever `StackOverflow`.

Compromis : une règle simple, la même pour toutes les instructions. Mais elle pose un rappel `SENSE` même quand sa
réponse ne pourra pas être empilée.

## 3. Ce que l'assistant IA a proposé

L'option B. Ses conséquences sont épinglées par des tests :
- `CONST` hors de la table sur une pile pleine donne `BadConst` (tâche T04, test `const_reads_the_table_before_pushing`) ;
- `CALL` sur une pile d'appels pleine avec une mauvaise cible donnera `CallStackOverflow` (T06) ;
- `SENSE` et `RAND` sur une pile pleine poseront le rappel, puis lèveront `StackOverflow` (T07).

**Aucun cas public ne confirme ce choix.** Il faut le confronter aux autres groupes à la séance d'interopérabilité (J4).

Ce que nous en avons vérifié : les trois tests qui fixent les conséquences du choix passent :
`const_reads_the_table_before_pushing` (T04), `call_pushes_its_frame_before_checking_the_target` (T06) et
`callbacks_on_a_full_stack_ask_then_overflow` (T07). Nous avons relu les cas publics qui fixent une partie de
l'ordre : 046b, 047b, 054b et 063 pour `StackUnderflow` en premier, et 032 pour `AlreadyActed` levée sans rappel.

## 4. Ce que nous avons décidé, et pourquoi

Nous gardons l'option B, « dépiler, calculer, empiler », y compris pour `SENSE` et `RAND` sur une pile pleine.

C'est une seule règle pour toutes les instructions, et elle se lit directement dans la notation `a b -- r` de B.3 : on
dépile les arguments, on fait le travail de l'instruction, puis on empile le résultat, et c'est cet empilement qui
peut lever `StackOverflow`. L'option A demanderait un ordre que la spécification n'écrit nulle part. Le prix : un
`SENSE` sur une pile pleine pose quand même sa question avant de faillir.

Aucun cas public ne tranche ces trois points. À la séance d'interopérabilité (J4), nous ferons tourner sur les
serveurs des autres groupes un `CONST` hors table sur une pile pleine, un `CALL` vers une mauvaise cible avec seize
appels en cours, et un `SENSE` sur une pile pleine, et nous comparerons les fautes et les rappels reçus. Si la
majorité fait autrement, nous réviserons cette entrée (section 5).

## 5. Ce qui nous ferait changer d'avis

Un cas caché, ou une majorité de serveurs à J4, qui ne pose pas le rappel sur une pile pleine. Il faudrait alors
réviser cette entrée et inverser le test de T07.

## À confronter à J4

Les questions à poser aux autres groupes à la séance d'interopérabilité, avec ce que fait notre VM aujourd'hui. On
lance `palestre corpus <cas> --server <leur adresse>` sur des cas écrits pour l'occasion, on note le jour même ce que
répond leur serveur, et on révise cette entrée si la majorité fait autrement.

- `SENSE` ou `RAND` sur une pile pleine (64 valeurs) : le rappel est-il posé avant le `StackOverflow` ? Nous : oui,
  la question part, puis c'est l'empilement de la réponse qui déborde.
- `ACT k` : dans quel ordre tombent `StackUnderflow`, `BadAction` et `AlreadyActed` ? Nous : l'arité d'abord, puis
  `k` dans `0..=3`, puis l'action déjà tentée, et seulement ensuite le rappel.
- `CONST k` hors de la table sur une pile pleine : `BadConst` ou `StackOverflow` ? Nous : `BadConst`.
- `CALL` vers une mauvaise cible avec seize appels en cours : `CallStackOverflow` ou `BadJump` ? Nous :
  `CallStackOverflow`.

Réponses des autres groupes : *à remplir le jour de J4.*

## Historique

- 2026-10-27 — proposée.
- 2026-11-03 — précisée par Farid GOUIRAH en écrivant les rappels (T07), sans changer la décision. `ACT k` vérifie,
  dans cet ordre : l'arité en dépilant `arg` (`StackUnderflow`), puis `k` dans `0..=3` (`BadAction`, sans rappel et
  sans action consignée, E.4), puis qu'aucune action n'a été tentée dans le tour (`AlreadyActed`, sans rappel, cas
  032). Seulement ensuite le rappel est posé, et l'issue empilée ne peut pas déborder puisque `arg` vient d'être
  dépilé (E.4). `SENSE` et `RAND` posent le rappel sans regarder la place sur la pile ; c'est l'empilement de la
  réponse qui lève `StackOverflow` (test `callbacks_on_a_full_stack_ask_then_overflow`).
- 2026-11-21 — adoptée.
- 2026-11-21 — ajout des questions à confronter à J4 (T11), par Farid GOUIRAH.

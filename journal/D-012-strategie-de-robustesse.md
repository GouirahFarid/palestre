# D-012 — Stratégie de robustesse : propriétés et fuzzing

- **Identifiant :** D-012
- **Date :** 2026-11-22
- **Auteurs :** Chaimae Faris
- **Statut :** adoptée
- **Commits concernés :** `ca1fbc5` (proposition), `2a5e6fe` (propriétés et cibles de fuzzing)

## 1. Contexte

Le README (§1) dit de l'invariant 5, l'absence de `panic`, qu'il « se teste par fuzzing, pas par relecture ». Les
invariants 1 (déterminisme) et 2 (terminaison) portent sur toute entrée, pas seulement sur celles qu'on a pensé à
écrire dans un test. Le README (§3) recommande `proptest`, `arbitrary` et `cargo-fuzz`.

Trois surfaces reçoivent des octets qu'on ne contrôle pas : le chargeur (`Program::load`), la machine (n'importe quel
code, n'importe quelles réponses aux rappels), et la connexion du serveur `palestre exec`.

## 2. Options envisagées

### Option A — `proptest` seul
Compromis : stable, lancé par `cargo test` donc en CI, et reproductible grâce aux graines de régression. Mais il tire
ses entrées au hasard selon nos stratégies, sans savoir quelles branches du code il a déjà visitées.

### Option B — `cargo-fuzz` seul
Compromis : guidé par la couverture du code, très efficace sur un décodeur. Mais il demande la chaîne nightly, et il
ne tourne pas dans la CI standard.

### Option C — les deux
`proptest` pour les **propriétés** (déterminisme, terminaison, aller-retour), et `cargo-fuzz` pour l'**absence de
`panic`** sur les trois surfaces d'entrée.

## 3. Ce que l'assistant IA a proposé

L'option C, avec trois cibles de fuzzing : `load` (le chargeur), `execute` (la machine) et `connection` (le serveur).
Chaque plantage trouvé devient un test de régression nommé, et son histoire est racontée dans cette entrée.

L'argument de terminaison s'écrit ici en toutes lettres, parce que les tests le confirment mais ne le remplacent pas :
chaque instruction autre que `HALT` coûte au moins 1 unité de gaz, et le gaz est prélevé avant l'exécution (B.2).
Un tour exécute donc au plus `budget` instructions qui coûtent, plus le `HALT` final : il s'arrête toujours.

Une propriété demande une précaution : « un budget plus grand ne change pas une exécution qui n'a pas manqué de
gaz » est fausse pour un programme qui exécute `GAS`, puisque `GAS` lit le gaz restant (B.7). Le test écarte donc les
programmes qui contiennent l'octet de `GAS` (`0x71`). C'est un faux positif écarté en connaissance de cause. Le plan
les rejetait avec `prop_assume!`, mais un programme au hasard sur cinq environ contient cet octet, et à 5000 cas
proptest abandonnait (« Too many global rejects ») : la stratégie les filtre maintenant elle-même.

L'assistant a écrit les tests et les cibles de fuzzing du plan, puis a vérifié qu'un test de la VM pouvait échouer.
Il a glissé un bug exprès dans `TRACE` (la valeur change quand le budget dépasse 1000) : la propriété sur le budget
passait quand même. Les programmes faits d'octets au hasard fautent presque tous dès leurs premiers octets, sur un
opcode inconnu (43 opcodes sur 256 valeurs) ou une pile vide, et n'atteignent presque jamais un `TRACE`. Les
propriétés de la VM tirent donc maintenant leurs programmes pour moitié en octets bruts, qui éprouvent le décodage,
et pour moitié en instructions valides, dont la moitié en `PUSHI16` pour nourrir la pile. Avec ce générateur, le bug
glissé est trouvé 5 fois sur 5 à 256 cas, et proptest le réduit à 4 octets : `PUSHI16 0`, `TRACE`. Le bug a ensuite
été retiré. L'assistant a aussi supprimé le fichier de régression qu'il avait laissé : il ne correspondait à aucun
vrai défaut.

Ce que nous en avons vérifié : `cargo test --workspace` passe (198 tests, dont les 9 propriétés), et
`PROPTEST_CASES=5000 cargo test --release` aussi, sans aucun contre-exemple. Nous avons lu les statistiques finales
des trois campagnes de fuzzing (voir l'historique) : aucun plantage, et `fuzz/artifacts/` est vide. Nous avons relu
dans `crates/vm/src/machine.rs` le prélèvement du gaz, `gas_left.checked_sub(instr.gas())`, fait avant d'exécuter
l'instruction : c'est la base de l'argument de terminaison. Et nous avons vu l'essai du bug glissé : raté avec les
octets bruts, trouvé avec le nouveau générateur.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons l'option C : `proptest` et `cargo-fuzz`.

Les propriétés tournent à chaque `cargo test`, donc à chaque pull request en CI : le déterminisme et la terminaison
sont revérifiés sans y penser, et un échec est réduit tout seul à un cas minimal. Le fuzzing, lui, cherche ce que
nous n'avons pas imaginé, sur les entrées que les autres groupes contrôleront à J4 : leurs fichiers `.pbc` et leurs
trames réseau. C'est ce que demande l'invariant 5.

La terminaison, elle, n'est pas seulement testée : elle se démontre par le gaz, comme écrit en section 3. Les tests la
confirment sur des millions de programmes, mais c'est l'argument qui la garantit.

Enfin, l'essai du bug glissé nous a appris qu'un test peut passer sans rien prouver. Nous gardons donc le générateur
plus fort, et non les octets bruts du plan.

## 5. Ce qui nous ferait changer d'avis

Une campagne de fuzzing de plusieurs heures qui ne trouve plus de nouvelle couverture : on arrêterait, en notant les
chiffres. Ou un plantage trouvé par le fuzzing que `proptest` aurait dû voir : il faudrait alors renforcer ses
stratégies.

## Historique

- 2026-11-22 — proposée.
- 2026-11-22 — premières campagnes de fuzzing, 30 minutes chacune en parallèle, nightly du 2026-11-21 et
  `cargo-fuzz` 0.13.2. Aucun plantage.
  - `load`, amorcée avec les 65 fichiers `program.pbc` du corpus public : 419 547 036 exécutions (232 952 par
    seconde), couverture de 96 arêtes, plate dès la quatrième minute.
  - `execute` : 11 963 292 exécutions (6 642 par seconde), 404 arêtes, contre 400 à la quatrième minute.
  - `connection` : 70 486 228 exécutions (39 137 par seconde), 1 771 arêtes, encore en hausse à la fin : c'est la
    campagne à prolonger en premier.
- 2026-11-22 — adoptée.

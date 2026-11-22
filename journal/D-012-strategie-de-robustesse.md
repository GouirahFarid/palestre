# D-012 — Stratégie de robustesse : propriétés et fuzzing

- **Identifiant :** D-012
- **Date :** 2026-11-22
- **Auteurs :** Chaimae Faris
- **Statut :** proposée
- **Commits concernés :** —

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
programmes qui contiennent l'octet de `GAS` (`0x71`). C'est un faux positif écarté en connaissance de cause.

Ce que nous en avons vérifié : *à compléter.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

Une campagne de fuzzing de plusieurs heures qui ne trouve plus de nouvelle couverture : on arrêterait, en notant les
chiffres. Ou un plantage trouvé par le fuzzing que `proptest` aurait dû voir : il faudrait alors renforcer ses
stratégies.

## Historique

- 2026-11-22 — proposée.

# D-011 — Après une faute, l'action tentée est gardée mais son effet est annulé

- **Identifiant :** D-011
- **Date :** 2026-11-22
- **Auteurs :** Farid GOUIRAH
- **Statut :** adoptée
- **Commits concernés :** `3d1a4bb` (proposition), `cba7178` (crate `arena`)

## 1. Contexte

E.2, étape 4 : si un agent faute, « l'état de l'agent et du monde est restauré depuis l'instantané. Ne sont pas
restaurés : le gaz consommé, la trace, et l'action tentée ».

La phrase se lit de deux façons. Un agent qui réussit un `MOVE` puis faute avant `HALT` reste-t-il sur sa nouvelle
case ? Ou bien revient-il à sa place, et seule la trace de son `MOVE` est-elle gardée ?

## 2. Options envisagées

### Option A — l'effet de l'action reste appliqué
L'agent reste déplacé, la ressource reste ramassée.

Compromis : mais alors « restaurer l'agent et le monde » ne restaurerait presque rien. En dehors d'`ACT`, un
programme ne peut changer que sa mémoire : la spec aurait simplement dit « la mémoire est restaurée ».

### Option B — l'enregistrement de l'action est gardé, son effet est annulé
Le journal du match et `RESULT.act` gardent l'action tentée, comme ils gardent le gaz et la trace, qui sont eux aussi
des enregistrements. Mais la position, l'énergie, la charge, les ressources et la mémoire reviennent à l'instantané.

Compromis : c'est cohérent avec `corpus/FORMAT.md`, où `act` est rapporté « même si l'exécution a fauté ensuite ».
Mais rien dans la spec ne le dit en toutes lettres.

## 3. Ce que l'assistant IA a proposé

L'option B : l'enregistrement est gardé, l'effet est annulé. Le test `a_fault_restores_the_agent_and_the_world` la
fixe : un agent écrit en mémoire, réussit un `MOVE`, puis faute ; il se retrouve à sa place, avec sa mémoire et son
énergie d'avant, mais le journal du tour garde le `MOVE` réussi. Le décrément d'énergie de fin de tour (étape 5)
s'applique, lui, même après une faute, comme le dit E.2.

Un détail voisin : `TAKE` sur une case vide réussit, en ramassant 0. E.4 ne lui donne aucun cas d'échec, au contraire
de `MOVE`, `DROP` et `EAT`.

Ce que nous en avons vérifié : `cargo test -p arena` passe, dont `a_fault_restores_the_agent_and_the_world` et
`a_fault_still_costs_the_turn_energy`. Nous avons relu `play_turn` dans `crates/arena/src/turn.rs` : sur une faute, le
monde entier revient à l'instantané, puis le journal du tour reçoit le gaz, la faute, l'action et la trace du rapport,
et l'énergie baisse de 1. Nous avons relu E.2 et E.4 dans la spec.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons l'option B : l'action tentée reste dans le journal et dans `RESULT.act`, mais son effet est annulé.

Avec l'option A, « restaurer l'agent et le monde » ne restaurerait presque que la mémoire, et la spec l'aurait dit
autrement. L'option B lit chaque mot de E.2 : ce qui est gardé, ce sont des enregistrements (le gaz, la trace,
l'action tentée), et tout le reste revient à l'instantané. Elle est aussi cohérente avec `corpus/FORMAT.md`, qui
rapporte `act` même quand l'exécution a fauté ensuite.

Nous gardons enfin la règle de `TAKE` : sur une case vide, il réussit en ramassant 0, puisque E.4 ne lui donne aucun
cas d'échec.

## 5. Ce qui nous ferait changer d'avis

Une réponse de l'enseignant, ou une majorité de groupes qui lit l'autre sens à J4. Le changement resterait local : ne
pas restaurer le monde quand le rapport contient une action.

## Historique

- 2026-11-22 — proposée.
- 2026-11-22 — adoptée.

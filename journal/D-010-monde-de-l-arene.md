# D-010 — Le monde de l'arène : carte, aléa et départ

- **Identifiant :** D-010
- **Date :** 2026-11-22
- **Auteurs :** Farid GOUIRAH
- **Statut :** adoptée
- **Commits concernés :** `52a8fc3` (proposition), `cba7178` (crate `arena`, agents, `palestre match`)

## 1. Contexte

L'annexe E commence à E.2 : il n'y a pas de E.1. La spec ne dit donc rien de la grille, des murs, des ressources de
départ, de l'énergie initiale, des positions de départ, de la numérotation des agents, ni d'où vient la valeur de
`RAND`. Seuls `turns_max = 500` (E.5), `eat_rate = 10` (E.4) et `gas_budget = 1000` (E.2) sont donnés.

Il faut pourtant tout cela pour jouer un match, et le README (§2) demande que deux matchs avec les mêmes entrées
donnent le même résultat.

## 2. Options envisagées

### Carte : générée à partir d'une graine, ou lue dans un fichier texte
Compromis : une carte générée donne de la variété sans effort, mais on ne la voit qu'en la jouant. Un fichier texte se
lit d'un coup d'œil, se versionne, et rejouer un match revient à donner le même fichier.

### Aléa de `RAND` : la crate `rand`, ou un SplitMix64 écrit à la main
Compromis : `rand` est autorisée (README §3), mais son algorithme peut changer d'une version à l'autre, et le même
match ne donnerait plus le même résultat après une mise à jour. SplitMix64 tient en une douzaine de lignes, ne change
jamais, et se teste sur sa suite de référence publiée.

### Identifiants : 0 et 1, ou 1 et 2
Compromis : 0 et 1 sont aussi les indices des deux agents dans nos tableaux. Le cas 207 du corpus répond `SENSE 6`
par `1` et `0`, ce qui laisse penser que la spec compte à partir de 0.

## 3. Ce que l'assistant IA a proposé

- Une carte lue dans un fichier texte : une ligne par rangée `y`, un caractère par case `x`. `.` est vide, `#` un mur,
  `A` et `B` les départs des agents 0 et 1, `1` à `9` une ressource de cette quantité. Toutes les lignes ont la même
  longueur, et le bord de la grille compte comme un mur (E.3).
- Une carte par défaut, `maps/default.map`, de 12 × 8 cases, symétrique par une rotation de 180° pour que les deux
  places se valent.
- Un SplitMix64 à graine explicite (`--seed`, 0 par défaut). Les deux agents tirent dans le même flux, dans l'ordre
  où leurs `RAND` sont posés.
- Les identifiants 0 et 1, et une énergie initiale de 100.
- Le générateur n'est **pas** remis en arrière quand un agent faute : il appartient à l'environnement, pas à l'agent
  ni au monde que E.2 restaure.

L'assistant a aussi écrit le code de la tâche T13 (`crates/arena` et `palestre match`) et les deux agents de
démonstration, `agents/idler.asm` et `agents/harvester.asm`. Les 21 tests de la carte et des règles sont passés du
premier coup. Le récolteur, lui, a d'abord perdu contre l'agent inerte : il tirait une nouvelle direction au hasard à
chaque tour, tournait en rond près de son coin, et mourait au tour 84 sans avoir trouvé les ressources du centre. Il
garde maintenant son cap dans `mem[3]` tant que la case devant lui est libre, et n'en tire un nouveau que s'il est
bloqué. Sur les graines 0 à 29, il gagne 29 matchs ; il perd celui de la graine 14, où il longe trop longtemps le bord
ouest avant de trouver à manger.

Ce que nous en avons vérifié : `cargo test -p arena -p palestre` passe (4 tests de la carte, 17 des règles, celui du
MVP et celui de `palestre match`). Nous avons joué un match à la main avec
`palestre match harvester.pbc idler.pbc --seed 1 --log match.json` : 100 tours, l'agent 0 gagne avec 8 charges et
aucune faute, et le journal est écrit. Nous avons relu `play_turn` dans `crates/arena/src/turn.rs` : instantané,
exécution, restauration sur faute sans toucher au générateur, décrément d'énergie même après une faute, puis la mort
qui dépose ce que l'agent porte. Nous avons relu l'annexe E, de E.2 à E.5.

## 4. Ce que nous avons décidé, et pourquoi

Nous gardons toute la proposition.

La carte est un fichier texte : on la lit d'un coup d'œil, et rejouer un match revient à redonner le même fichier et
la même graine. La carte par défaut est symétrique, pour qu'aucune des deux places ne soit avantagée.

Nous écrivons SplitMix64 nous-mêmes plutôt que de prendre `rand` : une mise à jour de la crate ne pourra jamais
changer le résultat d'un match, et le test `splitmix64_matches_the_reference_sequence` vérifie la suite de référence.
Le générateur n'est pas remis en arrière quand un agent faute, parce que E.2 restaure l'agent et le monde, et que le
générateur n'est ni l'un ni l'autre : il fait partie de l'environnement, comme le client qui fournit le hasard en K.1.

Les identifiants 0 et 1 suivent le cas 207 du corpus, et l'énergie de 100 laisse 100 tours à un agent qui ne fait
rien, ce qui suffit pour qu'un bon agent fasse la différence.

Nous gardons aussi la correction du récolteur : garder son cap est la plus petite règle qui le fait gagner, et un
agent de démonstration doit rester simple à lire.

## 5. Ce qui nous ferait changer d'avis

Une E.1 publiée dans une version suivante de la spec. Ou, à J4, un format de carte commun à plusieurs groupes pour
organiser des tournois croisés : on adopterait le leur.

## Historique

- 2026-11-22 — proposée.
- 2026-11-22 — adoptée. Jalon MVP atteint : `palestre match` oppose deux agents, et le
  récolteur bat l'agent inerte.

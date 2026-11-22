# D-010 — Le monde de l'arène : carte, aléa et départ

- **Identifiant :** D-010
- **Date :** 2026-11-22
- **Auteurs :** Farid GOUIRAH
- **Statut :** proposée
- **Commits concernés :** —

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

Ce que nous en avons vérifié : *à compléter.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

Une E.1 publiée dans une version suivante de la spec. Ou, à J4, un format de carte commun à plusieurs groupes pour
organiser des tournois croisés : on adopterait le leur.

## Historique

- 2026-11-22 — proposée.

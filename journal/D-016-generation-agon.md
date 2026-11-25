# D-016 — Génération de code `agon` : les variables en mémoire, à adresses fixes

- **Identifiant :** D-016
- **Date :** 2026-11-24
- **Auteurs :** Farid GOUIRAH
- **Statut :** adoptée
- **Commits concernés :** `c248828` (proposition), `88df3e5` (vérifications, génération et `palestre compile`)

## 1. Contexte

B3 (D-015) produit l'arbre d'un programme `agon` ; il reste à en tirer du bytecode `PBC`. La machine impose trois
choses. Elle n'a ni registres ni accès profond à la pile : `DUP`, `SWAP`, `OVER` et `ROT` n'atteignent que les trois
valeurs du sommet (B.3), donc une variable ne peut pas vivre « sous » d'autres valeurs. Sa mémoire fait 256 cellules,
**persistantes** d'un tour à l'autre (B.1). Et sa pile d'appels fait 16 cadres, qui ne contiennent que des adresses de
retour.

Il faut aussi vérifier ce que la grammaire ne voit pas : les noms inconnus, le nombre d'arguments, et les capteurs et
actions, dont le numéro est un opérande immédiat de `SENSE k` et `ACT k`, donc connu à la compilation.

## 2. Options envisagées

### Option A — les variables dans la pile
Garder chaque variable sur la pile et la ramener au sommet avec `SWAP`, `ROT` et `OVER`. Compromis : impossible
au-delà de trois valeurs de profondeur, donc dès qu'une expression utilise plus de deux ou trois variables.

### Option B — les variables en mémoire, à adresses fixes par fonction
Chaque paramètre et chaque `let` reçoit sa cellule, lue par `LOAD k` (3 de gaz) et écrite par `STORE k` (4).
Compromis : pas de récursion, puisque deux activations d'une même fonction partageraient leurs cellules.

### Option C — une pile de cadres simulée en mémoire
Un pointeur de cadre dans une cellule, et chaque variable à un décalage de ce pointeur. Compromis : la récursion
devient possible, mais chaque lecture coûte `LOAD fp ; PUSHI16 off ; ADD ; LOADX`, environ 10 de gaz au lieu de 3.

## 3. Ce que l'assistant IA a proposé

L'option B. Les cellules `128..=255` sont au compilateur, allouées de 255 vers le bas, une par paramètre ou `let` de
chaque fonction ; au-delà de 128 variables, c'est une erreur. Les cellules `0..=127` sont à l'agent, par `mem[…]` :
une adresse constante hors de `0..=127` est refusée, pour qu'un `mem[200] = …` n'écrase pas une variable en silence.
La récursion, directe ou mutuelle, est refusée : on cherche les cycles du graphe d'appel par un parcours en profondeur
à trois couleurs, et l'erreur nomme le cycle (`récursion interdite : f → g → f`). Sans cycle, la chaîne d'appels la
plus longue depuis `main` est connue : au-delà de 16 cadres, `main` compris, c'est aussi une erreur, plutôt qu'un
`CallStackOverflow` à l'exécution.

Les vérifications, avant de générer quoi que ce soit :

- les fonctions ont des noms uniques, et `main` existe, sans paramètre ;
- une variable est visible de son `let` à la fin de son bloc, et son nom est unique dans la fonction : une variable
  déclarée dans un `if` ne peut pas être lue après lui, où elle garderait la valeur d'un tour précédent ;
- les constantes s'évaluent à la compilation, avec l'arithmétique enveloppante de la machine (B.6), et ne peuvent
  citer que des constantes définies avant elles ; une variable ne peut pas porter le nom d'une constante ;
- les fonctions prédéfinies sont `sense(K)`, `act(K, arg)`, `rand()`, `gas()` et `trace(e)` ; leur arité est exacte,
  et `K` est une expression constante, au plus 255 pour `sense` et 3 pour `act`.

Les conventions de génération :

- le code commence par `CALL main ; HALT` ;
- un appel `f(a, b)` empile `a` puis `b`, et le prologue de `f` fait `STORE b ; STORE a`, dans l'ordre inverse ;
- toute fonction rend une valeur sur la pile : `return;` et la fin du corps rendent 0 ;
- une instruction-expression est suivie de `DROP`, sauf `trace(…)`, dont `TRACE` ne laisse rien ; en position
  d'expression, `trace(e)` vaut 0 ;
- chaque étiquette générée est suivie d'un `JUMPDEST` ;
- `<=` devient `GT ; NOT`, `>=` devient `LT ; NOT`, et `!=` devient `EQ ; NOT` ;
- un littéral sur `i16` devient `PUSHI16`, un autre une `.const` dédoublonnée et un `CONST k` ; un littéral précédé
  d'un `-` est un seul littéral négatif ;
- une constante `const` est remplacée par sa valeur ; les autres expressions ne sont pas pré-calculées.

`if` et `while` donnent :

```
; if c { A } else { B }          ; while c { A }
    <c>                          debut:
    JZ sinon                         JUMPDEST
    <A>                              <c>
    JMP fin                          JZ fin
sinon:                               <A>
    JUMPDEST                         JMP debut
    <B>                          fin:
fin:                                 JUMPDEST
    JUMPDEST
```

Le compilateur produit un texte assembleur (T12), une instruction par ligne et la ligne source en commentaire
(`; agon:12`), puis l'assemble. La preuve finale : un récolteur écrit en `agon` bat l'agent inerte.

Cette proposition s'écarte du plan sur un point, que l'assistant a corrigé avant d'écrire le code. Le plan évitait
aussi le `DROP` après `act(…)` en position d'instruction, comme après `trace(…)`. Or `ACT` empile toujours son issue
(B.3, `arg -- ok`), et cette valeur doit être jetée. L'erreur ne se serait jamais vue à l'exécution, puisqu'il n'y a
qu'une action par tour. Un test la fixe donc sur le texte assembleur : avec la convention du plan, c'est le seul des
tests qui échoue. Trois autres ajouts au plan sont dans la liste ci-dessus : la limite de 16 cadres, les adresses
`mem[…]` constantes, et la portée des variables limitée à leur bloc.

L'assistant a ensuite écrit les vérifications (`crates/agon/src/check.rs`), la génération (`codegen.rs`), la
sous-commande `palestre compile` et `agents/harvester.agon`. Le graphe d'appel se parcourt sans récursion, avec une
pile explicite : la hauteur de l'arbre est bornée par D-015, mais pas le nombre de fonctions. Une erreur de syntaxe
garde sa colonne dans le message (`ligne 2 : colonne 7 : …`) plutôt que de répéter la ligne, comme le prévoyait
le plan (`ligne 2 : 2:7 : …`).

Quatre sortes de tests le vérifient, en plus de ceux du plan. Chaque programme compilé par les tests passe aussi par
le vérificateur statique (D-014), qui doit le dire sûr. Deux propriétés comparent le calcul des constantes à celui de
la machine, pour les dix-sept opérateurs binaires et les trois unaires : `const C = a op b;` doit tracer la même valeur
que `a op b` calculé à l'exécution. Le récolteur compilé bat l'agent inerte, et il joue même exactement le même match
que `harvester.asm`, sur les graines 0, 1 et 7 : les journaux ne diffèrent que par le gaz consommé.

Le prix des variables en mémoire, mesuré avec `palestre run` dans les six situations de la stratégie :

| Situation                  | `harvester.asm` | `harvester.agon` | Écart  |
|----------------------------|-----------------|------------------|--------|
| manger                     | 43              | 61               | +42 %  |
| ramasser                   | 34              | 57               | +68 %  |
| aller vers un voisin (sud) | 158             | 272              | +72 %  |
| garder son cap             | 127             | 185              | +46 %  |
| prendre un nouveau cap     | 213             | 348              | +63 %  |
| attendre                   | 74              | 103              | +39 %  |

Les actions sont les mêmes dans les six cas, et le programme compilé fait 270 octets au lieu de 192. L'écart vient de
trois endroits. L'appel de `main` et son retour coûtent 12 de gaz par tour. Un test `x != 0` devient
`PUSHI16 0 ; EQ ; NOT` avant le `JZ`, soit 5 de plus que le `JZ` seul. Et un appel de `bit` coûte 40 de gaz, contre 15
pour le décalage écrit en ligne dans `harvester.asm`. Les tours restent loin du budget de 1000.

Ce que nous en avons vérifié : `cargo test --workspace` passe (270 tests), le vérificateur accepte chaque programme
compilé par les tests, et les deux propriétés passent aussi avec 20 000 cas chacune. Nous avons relevé le tableau de
gaz avec `palestre run`, dans les six situations : mêmes actions, de 39 à 72 % de gaz en plus. Nous avons comparé les
journaux des deux récolteurs contre l'agent inerte, graines 0, 1 et 7 : identiques au gaz près. Enfin, nous avons lu
la sortie de `palestre compile agents/harvester.agon --emit-asm` en face du source.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons l'option B : chaque paramètre et chaque `let` a sa cellule fixe, de 255 vers le bas, et la récursion
est refusée à la compilation.

C'est l'accès le moins cher : `LOAD k` et `STORE k` coûtent 3 et 4 de gaz, contre une dizaine avec un pointeur de
cadre simulé. La récursion ne nous manque pas : le récolteur n'en a pas besoin, et la refuser à la compilation nomme le
cycle, au lieu de laisser deux activations d'une même fonction partager leurs cellules. Plusieurs fautes d'exécution
deviennent des erreurs de compilation : plus de 16 cadres d'appel, une adresse `mem[…]` constante entre 128 et 255, une
constante divisée par zéro. Enfin, le récolteur compilé joue exactement le même match que `harvester.asm` sur trois
graines : la génération ne change aucune décision.

Nous en acceptons les limites :

- de 39 à 72 % de gaz en plus que le récolteur écrit à la main, et 270 octets au lieu de 192 ;
- pas de récursion, et 128 variables au plus pour tout le programme, dans les cellules 128 à 255 ;
- le compilateur ne borne pas la hauteur de pile d'une expression : `palestre verify` le fait sur le programme
  compilé, et les tests le lancent sur chacun de leurs programmes ;
- une adresse `mem[i]` calculée à l'exécution n'est pas vérifiée : un `i` entre 128 et 255 écraserait une variable.

## 5. Ce qui nous ferait changer d'avis

Un agent qui a besoin de récursion, ou de plus de 128 variables : on passerait à la pile de cadres simulée, en mesurant
le surcoût de gaz.

Un agent qui manquerait de gaz à cause de l'écart du tableau : on commencerait par les deux sources mesurées, le
`!= 0` avant un `JZ` et les petites fonctions comme `bit`, qu'on pourrait développer en ligne. Ce serait le bonus
« optimisations du compilateur », avec un test différentiel contre la version non optimisée.

## Historique

- 2026-11-24 — proposée.
- 2026-11-24 — adoptée.

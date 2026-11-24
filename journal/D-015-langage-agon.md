# D-015 — Langage `agon` : grammaire, précédence et analyse à la main

- **Identifiant :** D-015
- **Date :** 2026-11-23
- **Auteurs :** Chaimae Faris
- **Statut :** adoptée
- **Commits concernés :** `90d57f1` (proposition), `f61529c` (lexer, parseur et tests)

## 1. Contexte

Le bonus « langage source de haut niveau » (README §6) demande un compilateur vers `PBC`, pour un langage à nous
avec des variables, des boucles et des fonctions. « La syntaxe est la vôtre » : c'est le bonus qui laisse le plus de
liberté. Une contrainte, en revanche, ne se discute pas (README §3) : les générateurs de parseurs (`pest`, `nom`,
`lalrpop`, `chumsky`) sont interdits, le parseur s'écrit à la main.

Le travail est coupé en deux. B3 (cette entrée) écrit l'analyse lexicale et syntaxique, jusqu'à un arbre syntaxique
(AST) testé sous forme de S-expressions. B4 génère le bytecode à partir de l'arbre. Le langage doit aussi donner
accès à ce qu'un agent utilise vraiment : sa mémoire de 256 cellules, ses capteurs et ses actions. Or les masques de
`SENSE 8`, `9` et `13` (E.3) se testent bit à bit : `sense(8) & 2 == 0` sera une ligne courante.

## 2. Options envisagées

### Option A — précédence à la C
Les comparaisons lient plus fort que `&`, `^` et `|` : `x & 1 == 0` se lit `x & (1 == 0)`, donc `x & 0`. Compromis :
c'est l'ordre de C et de JavaScript, familier, mais c'est aussi un piège connu, et précisément sur la ligne qu'un agent
écrira le plus souvent.

### Option B — précédence à la Rust
Les opérateurs bit à bit lient plus fort que les comparaisons : `x & 1 == 0` se lit `(x & 1) == 0`. Compromis : moins
habituel pour qui vient de C, mais le test d'un masque s'écrit sans parenthèses.

Deux questions vont avec. Les comparaisons peuvent s'enchaîner (`a < b < c`, qui en C compare le booléen `a < b` à
`c`) ou être non associatives, comme en Rust, où c'est une erreur. Et le parseur peut être dirigé par une table de
précédences, ou écrit en descente récursive, une fonction par niveau.

## 3. Ce que l'assistant IA a proposé

La précédence à la Rust (option B), des comparaisons non associatives, et une descente récursive : une fonction par
ligne de la grammaire, chacune assez courte pour être expliquée au clavier. La grammaire :

```
module      := item*
item        := "const" IDENT "=" expr ";"
             | "fn" IDENT "(" (IDENT ("," IDENT)*)? ")" bloc
bloc        := "{" instr* "}"
instr       := "let" IDENT "=" expr ";"
             | IDENT "=" expr ";"
             | "mem" "[" expr "]" "=" expr ";"
             | "if" expr bloc ("else" (bloc | instr_if))?
             | "while" expr bloc
             | "return" expr? ";"
             | "halt" ";"
             | expr ";"
expr        := comparaison
comparaison := ou (("==" | "!=" | "<" | ">" | "<=" | ">=") ou)?      non associative
ou          := xou ("|" xou)*
xou         := et ("^" et)*
et          := decalage ("&" decalage)*
decalage    := somme (("<<" | ">>" | ">>>") somme)*
somme       := produit (("+" | "-") produit)*
produit     := unaire (("*" | "/" | "%") unaire)*
unaire      := ("-" | "!" | "~") unaire | primaire
primaire    := ENTIER | IDENT | IDENT "(" (expr ("," expr)*)? ")" | "mem" "[" expr "]" | "(" expr ")"
```

`instr_if` est la forme `if` de `instr` : `else if` n'est pas un mot-clé, c'est un `else` suivi d'un `if`. Tous les
opérateurs binaires sont associatifs à gauche : `1 - 2 - 3` vaut `(1 - 2) - 3`.

Le lexique : des entiers décimaux ou hexadécimaux (`0x1F`), qui doivent tenir dans un `i64` positif ; des
identifiants `[A-Za-z_][A-Za-z0-9_]*` ; des commentaires `//` jusqu'à la fin de la ligne. Les mots-clés sont `fn`,
`let`, `if`, `else`, `while`, `return`, `const`, `mem` et `halt`. Les opérateurs se reconnaissent du plus long au plus
court : `>>>` avant `>>`, avant `>=`, avant `>`.

`>>` est le décalage arithmétique (`SAR`), comme `>>` sur un `i64` en Rust, et `>>>` le décalage logique (`SHR`),
comme en Java. Il n'y a ni `&&` ni `||` : sur des booléens 0 ou 1, `&` et `|` suffisent, et il n'y a pas de
court-circuit. `!` est le non logique (`NOT`), `~` le complément bit à bit (`BNOT`), comme en B.6. Les capteurs, les
actions, `RAND`, `GAS` et `TRACE` ne sont pas des mots-clés : ce sont des appels de fonctions ordinaires
(`sense(8)`, `act(0, 1)`, `trace(x)`), que B4 reconnaîtra.

Une erreur de syntaxe donne la ligne et la colonne du lexème fautif, à partir de 1, et un message en français. Chaque
nœud de l'arbre garde sa ligne, pour les erreurs de B4. Pour les tests, l'arbre s'affiche en S-expressions :
`(+ 1 (* 2 3))`, `(let x e)`, `(= x e)`, `(mem= a e)`, `(if c (then …) (else …))`, `(while c (do …))`,
`(fn nom (a b) …)`, `(const N e)`.

L'assistant a ensuite écrit le lexer, l'arbre et le parseur, avec quatre ajouts au plan, chacun couvert par un test.
D'abord, une limite de 64 niveaux d'imbrication. L'affichage, la libération de l'arbre et bientôt la génération de
code le parcourent récursivement : sans limite, un source comme `((((…` ferait déborder la pile, et le compilateur
planterait au lieu de rendre une erreur. Les parenthèses, les blocs, les opérateurs unaires et chaque opérateur d'une
chaîne comme `1 + 1 + …` comptent pour un niveau. L'assistant a mesuré la pile en version de débogage : 127 niveaux
de parenthèses demandaient entre 1 et 2 Mio, trop près des 2 Mio d'un thread de test, alors que 64 tiennent dans
1 Mio. C'est le prix de la descente récursive : chaque parenthèse traverse onze fonctions, une par niveau de la
grammaire.

Les trois autres ajouts rendent les erreurs plus claires. `&&` et `||` donnent « « && » n'existe pas en agon : …
« & » suffit », au lieu d'une erreur de syntaxe sur le second `&`. Un nombre collé à des lettres (`12ab`) est mal
formé, plutôt que coupé en deux lexèmes, et un nombre trop grand a son propre message. Une erreur en fin de fichier
pointe la vraie fin du source (`1:23`), et pas le dernier lexème.

Un choix d'écriture, enfin : l'affectation se lit comme une expression suivie de `=`. La même règle couvre `x = …` et
`mem[…] = …`, sans regarder deux lexèmes à l'avance, et `a + b = 1` est refusé sur le `=`. Le résultat est celui de
la grammaire, à une différence près : `(x) = 1` est accepté, comme en Rust.

Ce que nous en avons vérifié : `cargo test --workspace` passe (243 tests, dont les 18 de `crates/agon`). Nous avons lu
les messages d'erreur sur une série d'exemples (`let = 3`, `trace(1) }`, `a < b < c`, `a && b`, `12ab`…), et nous les
trouvons clairs. Nous avons vu la mesure de la pile : 127 niveaux demandaient entre 1 et 2 Mio, 64 tiennent dans
1 Mio. Enfin, nous avons relu `crates/agon/src/parser.rs` en face de la grammaire, fonction par fonction.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons la précédence à la Rust, des comparaisons non associatives et une descente récursive, avec la grammaire
de la section 3.

Les agents testent sans cesse des bits de `SENSE 8`, `9` et `13`. Avec la précédence à la Rust, `sense(8) & 2 == 0` se
lit `(sense(8) & 2) == 0`, sans parenthèses, alors qu'en C il vaudrait `sense(8) & 0`. `a < b < c` devient une erreur
de syntaxe qui demande des parenthèses, au lieu de comparer en silence le booléen `a < b` à `c`. Chaque ligne de la
grammaire est une fonction courte de `parser.rs` : on peut expliquer la grammaire en lisant le code. Enfin, `&` et `|`
suffisent sur des booléens 0 ou 1 ; le court-circuit attendra un vrai besoin (section 5).

Nous gardons aussi les quatre ajouts de l'assistant : la limite de 64 niveaux, le message sur `&&` et `||`, les
erreurs sur les nombres, et les erreurs en fin de fichier.

## 5. Ce qui nous ferait changer d'avis

Des agents réels qui ont besoin du court-circuit, pour éviter un `SENSE` coûteux (8 de gaz) quand la première moitié
d'une condition suffit : on ajouterait `&&` et `||`, compilés en sauts.

Un vrai programme qui butterait sur la limite de 64 niveaux : on passerait les expressions en montée de précédence, un
appel par niveau d'imbrication au lieu d'onze, ce qui permettrait une limite plus haute pour la même pile.

## Historique

- 2026-11-23 — proposée.
- 2026-11-24 — adoptée.

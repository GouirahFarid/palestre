# D-015 — Langage `agon` : grammaire, précédence et analyse à la main

- **Identifiant :** D-015
- **Date :** 2026-11-23
- **Auteurs :** Chaimae Faris
- **Statut :** proposée
- **Commits concernés :** —

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

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

Des agents réels qui ont besoin du court-circuit, pour éviter un `SENSE` coûteux (8 de gaz) quand la première moitié
d'une condition suffit : on ajouterait `&&` et `||`, compilés en sauts.

## Historique

- 2026-11-23 — proposée.

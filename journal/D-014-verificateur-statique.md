# D-014 — Vérificateur statique : explorer les états abstraits

- **Identifiant :** D-014
- **Date :** 2026-11-23
- **Auteurs :** Farid GOUIRAH
- **Statut :** proposée
- **Commits concernés :** —

## 1. Contexte

Le bonus du README (§6) demande de prouver *avant toute exécution* qu'aucun saut n'atterrit hors du code ou sur une
cible invalide, et de borner la hauteur maximale de la pile. Le README (§1) le rapproche de la validation d'un module
WASM, et prévient que l'absence de table de destinations rend le problème « réellement intéressant ».

Trois choses compliquent l'analyse. D'abord, une cible n'est valide que par l'octet `0x43` qu'elle porte, même au
milieu d'un opérande (A.3, B.4) : le code n'a donc pas un découpage unique en instructions, puisqu'un saut peut
reprendre le décodage à n'importe quel octet. Ensuite, `JMPX` et `CALLX` sautent vers une valeur calculée. Enfin,
`RETURN` revient à l'adresse empilée par le `CALL`, sans marqueur. WASM a évité tout cela avec un flot de contrôle
structuré, l'EVM avec `JUMPDEST` et un balayage du code qui exclut les données des `PUSH`.

Un vérificateur doit être correct (*sound*) : toute faute qu'il prétend exclure ne doit jamais arriver à l'exécution.

## 2. Options envisagées

### Option A — un graphe de flot classique
Découper le code en blocs de base, puis calculer un intervalle de hauteur de pile par bloc. Compromis : c'est efficace,
mais cela suppose un découpage unique du code, qui n'existe pas ici. `RETURN` est mal traité : savoir où il revient
demande une analyse interprocédurale, avec des résumés par fonction.

### Option B — explorer l'espace des états abstraits
Un état abstrait est `(pc, hauteur de pile, pile des adresses de retour, action déjà tentée)`. Les valeurs sont
ignorées : `JZ` et `JNZ` mènent aux deux chemins, et `JMPX`/`CALLX` vers tout octet `0x43` du code. Chaque état est
décodé à son propre `pc`, si bien que les sauts au milieu d'un opérande sont suivis comme les autres, et les retours
exactement. Compromis : l'espace est fini (`pc` < 65 536, hauteur ≤ 64, au plus 16 retours), mais il peut être énorme
avec la récursion et `CALLX`. Une limite d'états garde le temps et la mémoire de l'analyse bornés.

## 3. Ce que l'assistant IA a proposé

L'option B, avec une file de travail et un `HashSet` des états déjà vus. Pour chaque état, le vérificateur rejoue les
vérifications de la machine dans le même ordre (D-005), sans les valeurs : le décodage, l'arité, les opérandes
statiques (`CONST k` hors de la table, `ACT k` inconnu, deuxième `ACT`), puis le débordement de pile, puis la pile
d'appels et les cibles. Une faute certaine arrête le chemin ; une faute possible, comme un `JZ` dont la cible est
mauvaise, est annoncée et les autres chemins continuent.

Le résultat est une liste d'`Issue` (une adresse et une faute *possible* à cette adresse), la hauteur de pile et la
profondeur d'appel maximales, le nombre d'états explorés, et un drapeau `complete` qui passe à faux si la limite
d'états (un million par défaut) a été atteinte. Un programme n'est sûr que si l'analyse est complète et ne trouve rien.

Les fautes qui dépendent des valeurs sont hors du périmètre : `DivByZero`, `OutOfBounds` de `LOADX`/`STOREX`, `BadConst`
de `CONSTX`, `OutOfGas`, et les fautes que l'environnement rend à un `SENSE` (`BadSensor`). `JMPX` et `CALLX` donnent
toujours un `BadJump` possible, puisque leur cible est une valeur.

La preuve est un test de propriété : sur du code tiré au hasard, toute faute structurelle qui arrive à l'exécution doit
avoir été annoncée. Un piège est signalé : `CALL` ne continue pas à l'instruction suivante, c'est le `RETURN` qui y
ramène. La prendre aussi comme successeur direct ferait apparaître de faux `StackUnderflow`.

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

Des programmes réels, comme les agents compilés en B4, qui atteignent la limite d'états à cause de la récursion ou de
`CALLX` : on passerait à des résumés par fonction.

## Historique

- 2026-11-23 — proposée.

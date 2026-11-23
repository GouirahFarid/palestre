# D-014 — Vérificateur statique : explorer les états abstraits

- **Identifiant :** D-014
- **Date :** 2026-11-23
- **Auteurs :** Farid GOUIRAH
- **Statut :** adoptée
- **Commits concernés :** `ad11a2b` (proposition), `17ef74a` (vérificateur, tests et sous-commande `verify`)

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

L'assistant a ensuite écrit le code, avec quatre écarts au plan. D'abord, `verify` dépend de `vm`, et pas seulement
de `pbc` : il lit `STACK_SIZE`, `CALL_DEPTH` et `ActKind::from_code` dans la machine, pour qu'une borne de B.1 ou une
action de E.4 ne se change qu'à un seul endroit. Ensuite, `Fault` dérive `Ord`, dans l'ordre de B.5, pour trier les
`Issue`.

Le troisième écart est le plus important. L'assistant a vérifié que le test de propriété pouvait échouer, en cassant
exprès le vérificateur de dix façons : `JZ` sans le chemin qui ne saute pas, `RETURN` qui ne mène nulle part ou un
octet trop loin, `CALL` qui continue aussi à l'instruction suivante, `JMPX` sans `BadJump`, des marqueurs oubliés,
le deuxième `ACT` non signalé ou l'action non retenue dans l'état, et les deux piles qui débordent un cran trop tard.
Avec les octets tirés uniformément du plan, la propriété n'attrapait aucun des sept premiers : 83 % des octets ne
sont pas des opcodes, et la plupart des tours s'arrêtaient sur le premier. Il a donc écrit un générateur de vrai code :
des instructions aux opérandes petits, des marqueurs fréquents, un `PUSHI16 67` dont l'opérande porte `0x43`, et
quelques valeurs empilées au départ. La propriété en attrape alors quatre. Les autres échappent pour une raison de
fond : elle vérifie que la *sorte* de faute a été annoncée, pas son adresse, et un programme tiré au hasard a des
fautes de toutes les sortes un peu partout. Deux tests unitaires épinglent donc l'adresse exacte :
`a_fault_right_after_a_return_is_reported` et `a_dynamic_jump_reaches_a_marker_inside_an_operand`. Chacun des dix
vérificateurs cassés est maintenant attrapé par au moins un test.

Enfin, la propriété vérifie avec une limite de 50 000 états : un programme tiré au hasard qui l'atteint est écarté de
toute façon, et avec la limite d'un million, 2 000 cas prenaient 50 secondes. Ils en prennent moins de 10 pour 5 000
cas.

Mesures, avec `palestre verify` : l'agent `harvester` donne 96 états, `idler` un seul, et le plus gros programme du
corpus public 65 ; tous en moins d'un centième de seconde. Un programme construit pour exploser (dix blocs
`JUMPDEST RAND CALLX`) atteint la limite d'un million d'états en 1,03 s et environ 280 Mo, en version optimisée. Sur le
corpus, `nominal/210-call-a-dispatch-table` est signalé : il appelle par `CALLX` une adresse lue dans sa table de
constantes, donc un `BadJump` reste possible pour le vérificateur.

Ce que nous en avons vérifié : `cargo test --workspace` passe (225 tests), et la propriété passe aussi avec
`PROPTEST_CASES=5000`. Nous avons vu les dix vérificateurs cassés exprès, chacun attrapé par au moins un test. Nous
avons lancé `palestre verify` sur les programmes du corpus et sur les deux agents : 023 donne `CallStackOverflow` en
`0x0005`, et 210 est signalé. Enfin, nous avons comparé `Analysis::step` (`crates/verify/src/lib.rs`) à
`Machine::exec` (`crates/vm/src/machine.rs`), vérification par vérification.

L'assistant a aussi fait le lien demandé par le README (§1). Avec le saut libre, chaque octet `0x43` du code peut être
une cible, même dans un opérande : il n'y a pas de découpage unique, et il faut explorer des états plutôt que des
blocs. L'EVM retrouve un découpage unique en excluant les données des `PUSH` des cibles valides, au prix d'un balayage
du code. WASM supprime la question : ses sauts visent des blocs imbriqués, et la validation se fait en une passe. Le
saut calculé reste le point dur partout : nous devons supposer que `JMPX` va n'importe où, et c'est pourquoi le format
EOF proposé pour l'EVM remplace ses sauts dynamiques par des sauts relatifs et des tables de sauts statiques.
Notre méthode ressemble surtout au vérificateur d'eBPF, qui explore lui aussi les chemins en écartant les états
déjà vus, et s'arrête au bout d'un million d'instructions examinées.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons l'option B : explorer tous les états abstraits atteignables depuis `pc = 0`, avec une limite d'un
million d'états.

C'est la seule des deux options qui convient à `PBC v1`. Chaque état est décodé à son propre `pc` : un saut sur un
`0x43` au milieu d'un opérande est suivi comme un autre, sans avoir à découper le code d'avance. La pile des adresses
de retour fait partie de l'état, donc `RETURN` revient exactement là où son `CALL` l'a dit, sans résumé par fonction.
La fonction qui applique une instruction (`Analysis::step`) reprend les vérifications de la machine dans le même ordre
(D-005) : elle est facile à expliquer et à comparer avec `Machine::exec`. Enfin, le coût mesuré est faible sur de vrais
programmes : 96 états pour l'agent `harvester`, 65 au plus sur le corpus, en moins d'un centième de seconde.

Nous acceptons quatre limites :

- les fautes qui dépendent des valeurs ne sont pas cherchées : `DivByZero`, `OutOfBounds` de `LOADX`/`STOREX`,
  `BadConst` de `CONSTX`, `OutOfGas` et `BadSensor` ;
- un saut calculé (`JMPX`, `CALLX`) donne toujours un `BadJump` possible : un programme correct comme
  `nominal/210-call-a-dispatch-table` est signalé ;
- à la limite d'un million d'états, l'analyse prend environ 280 Mo, parce que chaque état copie sa pile de retours ;
- la propriété vérifie la sorte de chaque faute, pas son adresse : ce sont les tests unitaires qui épinglent les
  adresses.

## 5. Ce qui nous ferait changer d'avis

Des programmes réels, comme les agents compilés en B4, qui atteignent la limite d'états à cause de la récursion ou de
`CALLX` : on passerait à des résumés par fonction. Si la mémoire devenait le problème, on partagerait les piles de
retour entre les états au lieu de les copier. Et s'il fallait accepter les tables de sauts comme celle de 210, on
suivrait les valeurs des constantes jusqu'au `JMPX` ou au `CALLX` qui les utilise.

## Historique

- 2026-11-23 — proposée.
- 2026-11-23 — adoptée.

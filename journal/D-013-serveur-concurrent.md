# D-013 — Serveur concurrent : un thread par connexion, rien de partagé

- **Identifiant :** D-013
- **Date :** 2026-11-22
- **Auteurs :** Chaimae Faris
- **Statut :** adoptée
- **Commits concernés :** `8689c9c` (proposition), `3f0983a` (serveur concurrent et tests)

## 1. Contexte

Notre serveur `palestre exec` est séquentiel (D-008) : un client qui se tait bloque tous les autres jusqu'au délai de
lecture. Servir plusieurs connexions à la fois est un bonus (README §6), à trois conditions : `std` seul, aucun état
partagé entre connexions, et un client lent ou tué en cours d'écriture ne gêne personne. Il se prouve par un test :
les transcripts de rappels et les `RESULT` doivent être identiques octet pour octet à ceux du serveur séquentiel.

Le README (§3) explique pourquoi rien n'oblige à partager : une connexion ne porte qu'une exécution à la fois, sans
pipelining (K.4), et le résultat ne dépend que de ce que le client a envoyé (K.5). Il ajoute qu'aucun verrou n'entre
dans la boucle d'exécution, et le §6 demande de savoir dire quel type traverse une frontière de thread, et pourquoi il
le peut.

## 2. Options envisagées

### Option A — `thread::spawn` par connexion
La fermeture passée à `spawn` doit être `Send + 'static` : on y déplace le `TcpStream` de la connexion (`Send`) et
une copie de `ServerConfig` (un `u32` et une `Option<Duration>`, donc `Send` aussi). Ni `Arc`, ni `Mutex`. Compromis :
le nombre de threads n'est pas borné.

### Option B — `thread::scope` autour de la boucle `incoming()`
Les threads peuvent emprunter `&ServerConfig` sans copie. Compromis : la portée ne se termine jamais, puisque le
serveur tourne indéfiniment ; et une panique d'un thread ne remonte qu'à la fin de la portée, donc jamais.

### Option C — un pool de N threads et un canal `mpsc`
Le nombre de threads est borné. Compromis : un client lent occupe un ouvrier entier, et le canal est un état partagé
entre les threads.

## 3. Ce que l'assistant IA a proposé

L'option A : `thread::spawn`, avec le flux déplacé dans le thread et la configuration copiée. Aucun état partagé,
comme le demande l'énoncé du bonus. Le nombre de threads n'est pas borné : c'est un risque de déni de service, à
documenter ici, sans contre-mesure dans ce bonus.

Le serveur séquentiel reste le comportement par défaut, et `palestre exec --concurrent` choisit le nouveau. Les deux
partagent le même code par connexion (`handle_connection`), qui ne change pas.

La preuve est un test de la crate `corpus`, qui dépend déjà d'`exec` : il rejoue toutes les exécutions du corpus
public contre les deux serveurs, enregistre chaque octet reçu, et compare. Trois autres tests : huit clients en
parallèle reçoivent les mêmes octets, un client muet ne retarde pas le suivant, et un client tué au milieu d'une trame
ne gêne personne.

L'assistant a ensuite écrit le code et les tests du plan, avec deux écarts. D'abord, `thread::Builder::new().spawn`
au lieu de `thread::spawn` : `spawn` panique quand le système refuse un thread de plus, c'est-à-dire précisément
quand trop de clients sont connectés. `Builder::spawn` rend une erreur, et la connexion est abandonnée avec une ligne
sur la sortie d'erreur. Ensuite, il a vérifié que les tests pouvaient échouer, en remplaçant un moment
`serve_concurrent` par le serveur séquentiel : seul le test du client muet échouait. Les trois autres passent aussi
avec un serveur séquentiel : la comparaison des transcripts est une équivalence, les huit clients parallèles font
simplement la queue, et un client tué ferme sa connexion tout de suite. Il a donc ajouté un test où le client
s'arrête au milieu d'un `SUBMIT` et reste connecté, qui échoue lui aussi contre le serveur séquentiel. Les deux
serveurs partagent `serve_one`, qui règle les délais de la connexion et appelle `handle_connection`.

Ce que nous en avons vérifié : `cargo test --workspace` passe (204 tests), et les tests de
`crates/corpus/tests/concurrent.rs` ont passé 20 fois de suite. Nous avons vu l'essai avec le serveur séquentiel à la
place : les tests du client muet et du client bloqué échouent, au bout des 30 secondes du délai de lecture du client.
Nous avons relu `serve_concurrent` : seuls le `TcpStream` et la copie de `ServerConfig` entrent dans le thread, et il
n'y a aucun `Arc`, `Mutex`, `RwLock` ni `mpsc` dans `crates/*/src`. Enfin, `palestre corpus corpus --server` contre
`palestre exec --concurrent` donne 65/65.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons l'option A : un thread par connexion, créé avec `thread::Builder`, et rien de partagé.

C'est ce que demande l'énoncé du bonus : aucun état ne passe d'une connexion à l'autre, il n'y a donc rien à
verrouiller, et aucun verrou ne peut approcher la boucle d'exécution. Le code d'une connexion, `handle_connection`, est
le même pour les deux serveurs et n'a pas changé : les octets envoyés sont les mêmes par construction, et le test le
confirme sur les 68 exécutions du corpus.

Les deux autres options ont chacune un défaut qui nous gêne. Avec `thread::scope`, la portée ne se termine jamais,
donc la panique d'un thread ne remonterait jamais. Avec un pool, un client lent occupe un ouvrier, et le canal serait
un état partagé, donc une décision à part entière.

Le nombre de threads n'est pas borné : c'est un risque de déni de service que nous acceptons pour ce bonus. Si le
système refuse un thread, la connexion est abandonnée, sans panique. Le serveur séquentiel reste le comportement par
défaut, et `--concurrent` le remplace.

### Les tests qui prouvent le bonus

Tous dans le commit `3f0983a` :

- `transcripts_are_byte_identical_to_the_sequential_server` (`crates/corpus/tests/concurrent.rs`) : les 68 exécutions
  du corpus public, octet pour octet, contre les deux serveurs.
- `parallel_clients_receive_the_same_bytes` : huit clients à la fois reçoivent les mêmes octets que du serveur
  séquentiel.
- `a_silent_client_does_not_block_anyone` et `a_client_stuck_mid_frame_does_not_block_anyone` : un client lent ne
  retarde pas le suivant. Ces deux-là échouent contre le serveur séquentiel.
- `a_client_killed_mid_frame_bothers_nobody` : le serveur continue après un client tué au milieu d'une trame.
- `concurrent_flag_serves_a_client_while_another_is_silent` (`crates/palestre/tests/cli_exec.rs`) : la même chose par
  la ligne de commande.

### Quel type traverse une frontière de thread

Deux valeurs, déplacées dans la fermeture passée à `Builder::spawn`, qui doit être `Send + 'static` : le `TcpStream`
de la connexion et une copie de `ServerConfig`. `TcpStream` est `Send` : il possède sa socket, et rien d'autre ne la
référence. `ServerConfig` est `Send` parce que tous ses champs le sont, un `u32` et une `Option<Duration>` : le
compilateur le déduit seul. Les deux sont possédées, donc `'static` : c'est pour cela qu'on copie la configuration
au lieu de passer `&ServerConfig`, un emprunt qui ne vivrait pas assez longtemps. Les sessions et les programmes ne
traversent jamais : `handle_connection` les crée dans le thread, et `Machine<'p>` y emprunte le programme. Un `Rc`, ou
un `&ServerConfig`, serait refusé par le compilateur.

## 5. Ce qui nous ferait changer d'avis

Un besoin de borner les connexions simultanées, par exemple un déni de service à J4 : on passerait au pool de
l'option C, dont le canal deviendrait le seul état partagé. Ce serait une décision à part entière, avec sa propre
entrée.

## Historique

- 2026-11-22 — proposée.
- 2026-11-22 — adoptée.

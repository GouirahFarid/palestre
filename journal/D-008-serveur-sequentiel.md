# D-008 — Un serveur séquentiel : identifiants de session et client muet

- **Identifiant :** D-008
- **Date :** 2026-11-21
- **Auteurs :** Chaimae Faris
- **Statut :** adoptée
- **Commits concernés :** `d0fd0aa` (proposition), `141a7ae` (serveur et commande `exec`)

## 1. Contexte

Le README (§2) accepte un serveur `palestre exec` séquentiel, qui sert une connexion après l'autre. K.4 laisse libre
la forme de l'identifiant de session : un compteur, une empreinte, un jeton, tant qu'il fait de 1 à 64 caractères
dans `[A-Za-z0-9_-]`.

K.8 ne prévoit aucun code pour un client qui ne répond plus (« un client qui se tait est une connexion qui s'en va »).
Or, avec un serveur séquentiel, un client muet bloque tous ceux qui attendent derrière lui.

## 2. Options envisagées

### Identifiants : un compteur décimal par connexion, ou l'empreinte SHA-256 du programme
Compromis : le compteur (`"1"`, `"2"`…) est le plus simple et deux `SUBMIT` donnent deux sessions distinctes.
L'empreinte (64 caractères hexadécimaux, pile la borne de K.4) permettrait un serveur sans état, mais deux `SUBMIT`
du même programme donneraient la même session, et il faudrait décider ce que cela veut dire pour `sessions_max`.

### Client muet : aucun délai, ou un délai de lecture qui ferme la connexion sans `ERROR`
Compromis : sans délai, le serveur reste conforme, mais un seul client endormi le bloque pour tout le monde. Un délai
débloque le serveur, au risque de couper un client légitime mais lent.

## 3. Ce que l'assistant IA a proposé

- Un compteur décimal par connexion, qui part de `1`. Les sessions sont rangées dans un `Vec<(String, Program)>`,
  limité à `sessions_max` (16 par défaut, annoncé dans `READY`). L'empreinte SHA-256 est quand même renvoyée dans
  `SESSION.pbc_sha256`, comme somme de contrôle.
- Un délai de lecture de 30 s par défaut, réglable par `--read-timeout`, et désactivé par `--read-timeout 0`. Il est
  posé par `TcpStream::set_read_timeout`. Quand il expire, la connexion est fermée sans `ERROR`, comme pour une
  coupure.
- Aucun état partagé entre deux connexions : chacune a ses propres sessions. C'est ce qui rendra le bonus B1 (serveur
  concurrent) presque gratuit.

L'assistant a aussi écrit le code de la tâche T10 (`crates/exec/src/server.rs` et le binaire `crates/palestre`)
pour faire passer les 25 tests du plan. Il y a ajouté deux règles que le plan ne donnait pas :

- le même délai vaut pour l'écriture (`set_write_timeout`) : un client qui ne lit plus un gros `RESULT` bloquerait
  lui aussi le serveur ;
- sur un `SUBMIT`, le chargement du programme est vérifié avant la limite de sessions : un programme invalide donne
  `bad_program` (la connexion se ferme) même si la liste est pleine. C'est la même idée que pour `EXEC` dans D-007 :
  le contenu d'abord, l'état ensuite.

Ce que nous en avons vérifié : `cargo test -p exec -p palestre` passe (24 tests du serveur, 1 test de la commande,
plus les 17 de T09). Nous avons lancé le serveur à la main :

    cargo run -p palestre -- exec --listen 127.0.0.1:9000

et vu la ligne `palestre exec : écoute sur 127.0.0.1:9000`. Nous avons relu la boucle `execute` de `server.rs` :
chaque `SENSE`, `ACT` ou `RAND` de la machine devient un aller-retour avec le client, et le `RESULT` n'est envoyé que
si tous les rappels ont abouti (K.6). Nous avons relu dans la spec la machine d'états de K.4 et les codes de K.8, en
particulier les deux qui laissent la connexion ouverte.

## 4. Ce que nous avons décidé, et pourquoi

Nous gardons le compteur décimal par connexion. C'est le plus simple à expliquer et à compter : deux `SUBMIT` font
deux sessions, et `sessions_max` se vérifie avec la longueur de la liste. L'empreinte reste envoyée dans
`pbc_sha256`, ce qui suffit au client pour vérifier que le serveur a chargé les bons octets.

Nous gardons le délai de 30 s, et `--read-timeout 0` pour s'en passer. Sans délai, un seul client endormi bloquerait
tous les autres, puisque le serveur est séquentiel. Quand le délai expire, la connexion est fermée sans `ERROR`, comme
le veut K.8 pour un client qui se tait.

Nous gardons aussi les deux règles ajoutées par l'assistant. Le délai en écriture ferme le même trou du côté d'un
client qui ne lit plus. Et vérifier le programme avant la limite suit l'ordre déjà choisi pour `EXEC` dans D-007 :
une requête mal formée ferme la connexion, quel que soit l'état.

## 5. Ce qui nous ferait changer d'avis

Un client légitime plus lent que 30 s entre deux trames, par exemple quelqu'un qui débogue pas à pas à J4 : il lui
suffit de lancer le serveur avec `--read-timeout 0`. Ou le besoin de rejouer un même programme sur plusieurs
connexions : il faudrait alors un cache indexé par empreinte, qui serait une décision à part entière (README §3).

## Historique

- 2026-11-21 — proposée.
- 2026-11-21 — adoptée.

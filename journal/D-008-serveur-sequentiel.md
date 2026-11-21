# D-008 — Un serveur séquentiel : identifiants de session et client muet

- **Identifiant :** D-008
- **Date :** 2026-11-21
- **Auteurs :** Chaimae Faris
- **Statut :** proposée
- **Commits concernés :** —

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

Ce que nous en avons vérifié : *à compléter.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

Un client légitime plus lent que 30 s entre deux trames, par exemple quelqu'un qui débogue pas à pas à J4 : il lui
suffit de lancer le serveur avec `--read-timeout 0`. Ou le besoin de rejouer un même programme sur plusieurs
connexions : il faudrait alors un cache indexé par empreinte, qui serait une décision à part entière (README §3).

## Historique

- 2026-11-21 — proposée.

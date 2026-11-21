# D-007 — Encodage JSON des trames et ordre de validation d'`EXEC`

- **Identifiant :** D-007
- **Date :** 2026-11-21
- **Auteurs :** Farid GOUIRAH
- **Statut :** adoptée
- **Commits concernés :** `2007a7d` (proposition), `7b5f5f9` (crate `exec`)

## 1. Contexte

K.0 demande que deux implémentations conformes échangent « des trames identiques », et K.5 qu'un `RESULT` rejoué soit
identique « octet pour octet ». Pourtant la spec ne fixe ni l'ordre des clés JSON, ni les blancs, ni ce qu'il faut
faire d'un champ inconnu : les exemples de K.3 sont indentés, mais rien ne dit qu'il faut les écrire ainsi.

K.8 laisse aussi un trou. Un `EXEC` peut être faux sur plusieurs points à la fois, par exemple une session inconnue
(`no_such_session`, la connexion reste ouverte) et une `mem` de 10 cases (`bad_memory`, la connexion se ferme). La
spec ne dit pas lequel des deux codes répondre.

## 2. Options envisagées

### Émission : JSON compact, ou JSON indenté comme les exemples
Compromis : le compact est plus court et n'a qu'une forme possible pour une valeur donnée, ce qui rend le « octet pour
octet » de K.5 facile à tenir. L'indenté ressemble aux exemples, mais il faut choisir une indentation que la spec ne
donne pas.

### Lecture : tolérante (champ inconnu ignoré), ou stricte (champ inconnu refusé en `bad_frame`)
Compromis : strict attrape les fautes de frappe, comme pour `case.toml` (D-006). Mais ici, en face, il y a le client
ou le serveur d'un autre groupe, qui peut très bien ajouter un champ à lui. Être strict ferait échouer J4 pour un
détail que la spec n'interdit pas.

### Ordre de validation d'`EXEC` : la session d'abord, ou le contenu d'abord
Compromis : vérifier la session d'abord rend `no_such_session` dès que la session est inconnue, même si le reste est
faux, et la connexion reste ouverte. Vérifier le contenu d'abord ferme la connexion dès que la requête est mal
formée, même si sa session existe.

## 3. Ce que l'assistant IA a proposé

- Émettre du JSON compact, avec les clés dans l'ordre des exemples de K.3. Avec `serde_json`, il suffit de déclarer
  les champs du `struct` dans cet ordre.
- Lire de façon tolérante : un champ inconnu est ignoré, selon le principe de robustesse (« strict dans ce qu'on
  envoie, tolérant dans ce qu'on reçoit »). Les règles que la spec pose restent vérifiées : un `SENSED` avec `value`
  et `fault` à la fois, ou sans aucun des deux, est refusé.
- Valider `EXEC` dans cet ordre : d'abord la forme du JSON et l'identifiant de session (`bad_frame`), puis `mem`
  (`bad_memory`), puis `budget` (`bad_budget`), et seulement ensuite l'existence de la session
  (`no_such_session`). Une requête mal formée n'est jamais traitée comme un refus dont on peut se remettre.
- Lire `budget` en `i64`, et non en `u16`, pour qu'un budget de `-1` ou de `70000` donne `bad_budget` et non
  `bad_frame`.

L'assistant a aussi écrit le code de la tâche T09 (`crates/exec`) pour faire passer les 17 tests du plan. Il y a
ajouté un détail que le plan ne donnait pas : `write_frame` refuse une charge utile de plus de `u32::MAX` octets
(erreur `InvalidInput`), au lieu de tronquer sa longueur en silence.

Ce que nous en avons vérifié : `cargo test -p exec` passe (17 tests : 7 sur les trames, 10 sur les messages). Nous
avons relu `read_frame` dans `crates/exec/src/frame.rs` : la longueur est comparée à la borne avant le `vec![0; len]`,
donc une trame qui annonce 4 Go est refusée sans rien allouer. Nous avons relu K.2, K.3 et K.8 dans la spec, et
vérifié à la main que les chaînes attendues dans `tests/message.rs` (`READY`, `RESULT`, `SENSED`) suivent l'ordre des
clés des exemples de K.3.

## 4. Ce que nous avons décidé, et pourquoi

Nous écrivons du JSON compact, avec les clés dans l'ordre de K.3. Une valeur n'a qu'une seule écriture compacte,
donc un `RESULT` rejoué est identique octet pour octet, comme le demande K.5.

À la lecture, un champ inconnu est ignoré. Pour `case.toml` nous avions choisi l'inverse (D-006), mais ce sont nos
propres fichiers. Ici, en face, il y a le client ou le serveur d'un autre groupe : refuser un champ que la spec
n'interdit pas ferait échouer J4 pour rien. Les règles que la spec pose, elles, restent vérifiées.

`EXEC` est validé dans l'ordre proposé : forme et identifiant de session, puis `mem`, puis `budget`, puis l'existence
de la session. Une requête mal formée ferme la connexion, même si sa session existe : elle ne doit pas passer pour un
refus dont le client peut se remettre. Pour la même raison, `budget` est lu en `i64`, afin qu'un `-1` ou un `70000`
donne `bad_budget` et non une erreur de JSON.

Cet ordre sera appliqué par le serveur en T10 ; T09 ne fournit que les types et `valid_session_id`.

## 5. Ce qui nous ferait changer d'avis

À J4, un client d'un autre groupe qui compare les octets du JSON au lieu de sa valeur, ou une majorité de serveurs qui
répond `no_such_session` d'abord. On s'alignerait sur eux et on réviserait cette entrée.

## À confronter à J4

Les questions à poser aux autres groupes à la séance d'interopérabilité, avec ce que fait notre serveur aujourd'hui. On
lance `palestre corpus <cas> --server <leur adresse>` sur des cas écrits pour l'occasion, on note le jour même ce que
répond leur serveur, et on révise cette entrée si la majorité fait autrement.

- Un `EXEC` avec une session inconnue et une `mem` de mauvaise taille : `no_such_session` ou `bad_memory` ? Nous :
  `bad_memory`, puis la connexion se ferme.
- Un champ JSON inconnu dans une trame : ignoré ou `bad_frame` ? Nous : ignoré.
- Leur client compare-t-il notre `RESULT` à l'octet près, ou seulement sa valeur ? Nous écrivons du JSON compact, avec
  les clés dans l'ordre de K.3.

Réponses des autres groupes : *à remplir le jour de J4.*

## Historique

- 2026-11-21 — proposée.
- 2026-11-21 — adoptée.
- 2026-11-21 — ajout des questions à confronter à J4 (T11).

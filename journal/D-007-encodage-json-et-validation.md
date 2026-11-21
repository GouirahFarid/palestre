# D-007 — Encodage JSON des trames et ordre de validation d'`EXEC`

- **Identifiant :** D-007
- **Date :** 2026-11-21
- **Auteurs :** Farid GOUIRAH
- **Statut :** proposée
- **Commits concernés :** —

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

Ce que nous en avons vérifié : *à compléter.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

À J4, un client d'un autre groupe qui compare les octets du JSON au lieu de sa valeur, ou une majorité de serveurs qui
répond `no_such_session` d'abord. On s'alignerait sur eux et on réviserait cette entrée.

## Historique

- 2026-11-21 — proposée.

# D-013 — Serveur concurrent : un thread par connexion, rien de partagé

- **Identifiant :** D-013
- **Date :** 2026-11-22
- **Auteurs :** Chaimae Faris
- **Statut :** proposée
- **Commits concernés :** —

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

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger.*

## 5. Ce qui nous ferait changer d'avis

Un besoin de borner les connexions simultanées, par exemple un déni de service à J4 : on passerait au pool de
l'option C, dont le canal deviendrait le seul état partagé. Ce serait une décision à part entière, avec sa propre
entrée.

## Historique

- 2026-11-22 — proposée.

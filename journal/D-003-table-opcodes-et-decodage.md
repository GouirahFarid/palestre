# D-003 — Une table d'opcodes unique et un décodage à la volée

- **Identifiant :** D-003
- **Date :** 2026-10-26
- **Auteurs :** Farid GOUIRAH
- **Statut :** adoptée
- **Commits concernés :** `d8b3040` (proposition), `cbd4552` (table et décodeur)

## 1. Contexte

Les annexes A.3 et B.4 autorisent un saut à atterrir au milieu d'un opérande, pourvu que l'octet visé vaille `0x43`
(`JUMPDEST`) : le décodage reprend alors à cet octet. L'annexe H.2 annonce une version 2 qui modifiera « un petit
nombre d'éléments » de la spécification, typiquement un opcode ou un coût en gaz.

## 2. Options envisagées

### Option A — pré-décoder le code au chargement
Transformer le code en `Vec<Instr>` une fois pour toutes.

Compromis : rapide à l'exécution, mais faux ici. Une cible au milieu d'un opérande n'a pas d'entrée dans le vecteur,
et il faudrait un second index « octet → instruction » pour la retrouver.

### Option B — décoder à la volée, à la position `pc`, à chaque pas
Compromis : fidèle à A.3, sans structure annexe, et simple à fuzzer. Le coût est un `match` par pas, négligeable pour
un budget de 1000 unités de gaz.

### Pour la table elle-même
Soit un `match` par propriété, dispersé dans le code (octet, mnémonique, opérande, gaz), soit **une seule table**
(`Op`) dont tout le reste dérive.

## 3. Ce que l'assistant IA a proposé

Le décodage à la volée et une table unique dans `op.rs` : `Op::from_byte` est un `match` sur les 43 octets, et une
fonction privée porte mnémonique, type d'opérande et gaz dans un seul `match`. Le gaz n'existe qu'à cet endroit, ce
qui localise une future règle de gaz v2. Un test vérifie que les 43 octets font l'aller-retour. `Instr` porte des
opérandes typés (`PushI16(i16)`, `Load(u8)`…), si bien qu'un opérande du mauvais type est irreprésentable.

L'assistant a aussi écrit le code (`crates/pbc/src/fault.rs`, `op.rs` et `instr.rs`, commit `cbd4552`) pour faire
passer les onze tests de `crates/pbc/tests/fault.rs` et `decode.rs`, eux aussi tirés de son plan. `decode` applique
les étapes 1 à 3 de B.2 dans l'ordre, et ne lit les octets qu'à travers `get` : aucun décalage, même `usize::MAX`, ne
peut le faire paniquer.

Ce que nous en avons vérifié : les onze tests de `crates/pbc/tests/fault.rs` et `decode.rs` passent, dont
`the_table_has_43_opcodes_and_bytes_round_trip` (chacun des 43 octets donne un `Op` qui revient au même octet). Le cas
006, un saut qui atterrit sur un `0x43` au milieu de l'opérande d'un `PUSHI16`, passe côté décodeur
(`decoding_resumes_inside_an_operand`) et côté VM (`a_marker_inside_an_operand_is_a_valid_target`). Nous avons relu
`crates/pbc/src/op.rs` : les coûts en gaz n'y sont écrits qu'à un seul endroit.

## 4. Ce que nous avons décidé, et pourquoi

Nous retenons les deux propositions : le décodage à la volée et une table d'opcodes unique.

Décoder tout le programme au chargement ne marche pas avec A.3 et B.4 : un saut peut viser un octet au milieu d'un
opérande, et une liste d'instructions pré-décodées n'a pas d'entrée à cet endroit. En décodant à la position `pc` à
chaque pas, ce cas n'a rien de particulier. Le prix est un `match` par pas, ce qui ne compte pas pour un budget de
1000 unités de gaz.

La table unique nous prépare à la version 2 annoncée en H.2 et révélée en soutenance : si un opcode ou un coût en gaz
change, on le modifie à un seul endroit, dans `op.rs`, et le reste du code suit.

## 5. Ce qui nous ferait changer d'avis

Un profil de performance (bonus d'optimisation, `criterion`) montrant que le décodage domine le temps d'exécution. Un
cache de décodage indexé par octet resterait compatible avec A.3.

## Historique

- 2026-10-26 — proposée.
- 2026-11-21 — adoptée.

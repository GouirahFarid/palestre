# Reproduire nos résultats

## Prérequis

- Rust stable, 1.85 au moins (la version minimale du workspace). Testé avec `rustc 1.98.1 (48a229cea 2026-09-01)` sur
  Linux.
- `bash`, pour le script de démonstration.
- Pas de réseau, une fois les dépendances téléchargées : `cargo fetch` la veille, puis tout marche hors ligne.

## En une commande

```bash
./scripts/demo.sh
```

Le script compile en `--release`, assemble les deux agents de démonstration, joue le match, rejoue le corpus public
deux fois (dans notre VM, puis contre notre serveur `palestre exec` lancé sur un port libre), et lance toute la suite
de tests. Il s'arrête à la première erreur, et finit par :

```
== Tout est vert.
```

avec le code de sortie 0. Les fichiers produits (agents assemblés, journal du match, sortie du serveur) sont dans
`target/demo/`. Hors ligne, depuis un dossier `target/` vide, il prend une vingtaine de secondes sur notre machine :

```bash
cargo fetch                             # la veille, avec le réseau
CARGO_NET_OFFLINE=true ./scripts/demo.sh
```

## Les commandes une à une

Après `cargo build --release`, le binaire est `target/release/palestre` :

```bash
export PATH="$PWD/target/release:$PATH"
```

### `palestre match` : le match de démonstration

```bash
palestre asm agents/harvester.asm -o harvester.pbc
palestre asm agents/idler.asm -o idler.pbc
palestre match harvester.pbc idler.pbc --seed 1 --log match.json
```

```
tours joués : 100
agent 0 : (1, 6) énergie 49, porté 8, vivant, 0 faute(s)
agent 1 : (11, 7) énergie 0, porté 0, mort, 0 faute(s)
vainqueur : agent 0
```

Le récolteur bat l'agent inerte : celui-ci ne mange jamais, et meurt de faim au tour 100. Le même `--seed` redonne le
même match, octet pour octet dans `match.json`. Options : `--map`, `--turns`, `--energy` (`palestre match --help`).

### `palestre corpus` : le corpus public

```bash
palestre corpus corpus
```

Une ligne `OK  <cas>` par cas, puis `65/65 cas conformes`. Le code de sortie vaut 1 dès qu'un cas échoue, et la ligne
`KO` dit lequel et pourquoi.

### `palestre exec` et `palestre corpus --server` : le protocole de l'annexe K

Dans un premier terminal :

```bash
palestre exec --listen 127.0.0.1:9000
```

```
palestre exec : écoute sur 127.0.0.1:9000
```

Le serveur traite une connexion après l'autre (D-008), et écrit une ligne par connexion terminée sur la sortie
d'erreur. Dans un second terminal, le même corpus, cette fois par le réseau :

```bash
palestre corpus corpus --server 127.0.0.1:9000
```

On attend encore `65/65 cas conformes`. Pour tester le serveur d'un autre groupe, il suffit de changer l'adresse.

### `palestre asm` et `palestre disasm` : l'assembleur

```bash
cat > add.asm <<'EOF'
    LOAD 0
    PUSHI16 3
    ADD
    TRACE
    HALT
EOF
palestre asm add.asm -o add.pbc
palestre disasm add.pbc
```

`asm` n'affiche rien quand tout va bien. `disasm` donne un texte qui se réassemble à l'identique, avec l'adresse de
chaque instruction :

```
    LOAD 0          ; 0x0000
    PUSHI16 3       ; 0x0002
    ADD             ; 0x0005
    TRACE           ; 0x0006
    HALT            ; 0x0007
```

Une erreur s'affiche `fichier:ligne : message`, par exemple `add.asm:2 : mnémonique inconnu « FOO »`, avec le code
de sortie 1. La syntaxe est dans `journal/D-009-syntaxe-assembleur.md`.

### `palestre verify` : le vérificateur statique

```bash
palestre verify add.pbc
palestre verify corpus/edge/023-call-stack-overflow/program.pbc
```

```
pile max : 2, appels max : 0, états : 5
sûr : aucune faute structurelle possible
```

```
0x0005 CallStackOverflow
pile max : 0, appels max : 16, états : 33
```

Une ligne par faute possible, avec son adresse, puis les bornes ; le code de sortie vaut 1 dès qu'une faute est
possible. Les fautes qui dépendent des valeurs (`DivByZero`, `OutOfGas`…) ne sont pas cherchées, et un `JMPX` ou un
`CALLX` donne toujours un `BadJump` possible. Le détail est dans `journal/D-014-verificateur-statique.md`.

### `palestre run` : un tour en local

```bash
palestre run add.pbc --mem 0=2 --budget 10
```

```
{"calls":[],"result":{"act":null,"fault":null,"gas_used":7,"mem":[2,0,…],"trace":[5]}}
```

(la mémoire complète a 256 cases). C'est la forme d'une exécution de `expected.json` dans le corpus. Les rappels se
scriptent : `--sense K=V` (un capteur absent répond `BadSensor`), `--act true|false`, `--rand V` (répétable).

## Les tests

```bash
cargo test --workspace                          # 225 tests, dont 11 propriétés proptest
PROPTEST_CASES=5000 cargo test --release        # les propriétés, plus longuement
```

Le fuzzing demande la chaîne nightly et `cargo-fuzz`, il ne fait pas partie de la démonstration :
`cd fuzz && cargo +nightly fuzz run load` (ou `execute`, `connection`). Les campagnes déjà faites sont dans
`journal/D-012-strategie-de-robustesse.md`.

# Architecture

## Les crates

Le dépôt est un workspace Cargo (D-001). Chaque crate a un rôle, et les dépendances ne vont que vers le bas. Le
schéma montre les principales, le tableau les donne toutes :

```
                    palestre            dépend des six autres
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
     corpus          arena           asm
        │              │              │
        ▼              │              │
      exec             │              │
        │              │              │
        ├────► vm ◄────┘              │
        │      │                      │
        ▼      │                      │
       pbc ◄───┴──────────────────────┘
```

| Crate      | Dépend de         | Rôle                                                                                   |
|------------|-------------------|----------------------------------------------------------------------------------------|
| `pbc`      | —                 | le format : chargement d'un `.pbc` (annexe A), table des opcodes et décodage (B.7), les 16 fautes |
| `vm`       | `pbc`             | la machine : cycle de B.2, gaz, pile, mémoire de 256 cases, rappels rendus comme des jetons (`Step`) |
| `asm`      | `pbc`             | l'assembleur et le désassembleur (D-009)                                                |
| `exec`     | `pbc`, `vm`       | l'annexe K : trames (K.2), messages JSON, serveur séquentiel, client                    |
| `corpus`   | `pbc`, `vm`, `exec` | la lecture des cas du corpus et leur rejeu, dans notre VM ou contre un serveur       |
| `arena`    | `pbc`, `vm`       | le match : carte, déroulement d'un tour (E.2), capteurs, actions, journal du match       |
| `palestre` | toutes            | le binaire : `exec`, `corpus`, `asm`, `disasm`, `run`, `match`                          |

`pbc` et `vm` ne dépendent de rien qui touche au monde extérieur : ni réseau, ni horloge, ni source d'aléa. Seul `exec`
ouvre des sockets, et la seule source d'aléa est le `SplitMix64` de l'arène, tiré d'une graine explicite (D-010).

## Le trajet d'un `EXEC`

Un client a déjà ouvert la connexion (`OPEN`, `READY`) et soumis son programme (`SUBMIT`, `SESSION`). Il envoie
maintenant `EXEC`.

1. **La trame.** `exec::server::serve` accepte les connexions une par une, et `handle_connection` crée la
   `Connection` qui garde les sessions. Dans `Connection::run`, `receive` appelle `frame::read_frame` avec la borne
   `CLIENT_MAX_LEN` : un type inconnu ou une longueur hors borne donne `bad_frame` avant toute lecture du contenu
   (K.2).
2. **Le message.** `Connection::exec` décode le JSON en `message::Exec` (`parse`, sinon `bad_frame`), puis vérifie dans
   l'ordre de D-007 : l'identifiant de session (`valid_session_id`), les 256 cases de `mem` (`bad_memory`), le budget
   dans `1..=65535` (`bad_budget`), et enfin l'existence de la session (`no_such_session`, qui laisse la connexion
   ouverte).
3. **La machine.** `execute` crée `vm::Machine::new(program, mem, budget)` et appelle `run`. `Machine::run`
   (`crates/vm/src/machine.rs`) applique B.2 à chaque pas : décoder, prélever le gaz avec
   `gas_left.checked_sub(instr.gas())`, exécuter, avancer.
4. **Un rappel.** Sur un `SENSE`, la machine s'arrête et rend `Step::Sense(SenseCall)` : le jeton contient la machine
   elle-même. Le serveur envoie la trame `SENSE {k}` (`SenseReq`) et attend la réplique avec
   `reply::<Sensed>(FrameType::Sensed)` ; toute autre trame donne `unexpected_message`. `Sensed::to_answer` en fait
   une valeur ou une faute, et `SenseCall::answer` la rend à la machine, qui repart et rend le `Step` suivant.
5. **Les autres rappels.** Même chemin pour `ACT`/`ACTED` (`ActReq::from_action`, puis `ActCall::answer(acted.ok)`)
   et pour `RAND`/`DREW` (`RandCall::answer(drew.value)`).
6. **Le résultat.** Au `HALT` ou à la première faute, `Machine::run` rend `Step::Done(Report)`. Le serveur le convertit
   avec `ResultMsg::from_report` et l'envoie dans une trame `RESULT` (`write_frame`).

Le jeton impose l'ordre du protocole par les types (D-004) : `answer` consomme le jeton, donc on ne peut ni répondre
deux fois au même rappel, ni relancer la machine sans répondre. Le programme n'est jamais copié : `Machine<'p>`
l'emprunte à la session pendant toute l'exécution.

Le même mécanisme sert hors du réseau. `vm::run_with` (`crates/vm/src/env.rs`) pilote une machine avec un `Env` qui
répond toujours tout de suite : l'arène (`TurnEnv`, dans `arena::Arena::play_turn`), le rejeu du corpus
(`corpus::InProcess` et `Replay`) et `palestre run` (`Scripted`).

## Les invariants

Où chacun des six invariants du README (§1) est garanti dans le code, et quel test le prouve.

| Invariant | Où il est garanti | Ce qui le prouve |
|-----------|-------------------|------------------|
| 1. Déterminisme | `pbc` et `vm` n'utilisent ni horloge, ni aléa, ni réseau. `RAND` sort de la VM par `Step::Rand` (`crates/vm/src/machine.rs`) : c'est l'environnement qui répond. Dans l'arène, ce générateur est `SplitMix64::new(config.seed)` (`crates/arena/src/rng.rs`, créé dans `Arena::new`). | `execution_is_deterministic` (`crates/vm/tests/props.rs`) ; `matches_are_deterministic_and_their_log_round_trips` et `splitmix64_matches_the_reference_sequence` (`crates/arena/tests/rules.rs`) |
| 2. Terminaison | `Machine::run` (`crates/vm/src/machine.rs`) prélève `gas_left.checked_sub(instr.gas())` avant d'exécuter, sinon `OutOfGas`. `HALT` est le seul des 43 opcodes à coûter 0 dans `Op::info` (`crates/pbc/src/op.rs`). | `any_program_terminates_within_its_budget` (`crates/vm/tests/props.rs`) ; `halt_still_runs_with_no_gas_left` (`crates/vm/tests/core.rs`) ; l'argument écrit dans D-012 |
| 3. Atomicité du tour | `Arena::play_turn` (`crates/arena/src/turn.rs`) prend `snapshot = self.world.clone()` avant la machine et remet `self.world = snapshot` après une faute ; le gaz, la trace et l'action tentée vont quand même au journal. La VM, elle, ne restaure rien : elle rend la mémoire telle quelle dans le `Report`. | `a_fault_restores_the_agent_and_the_world` et `a_fault_still_costs_the_turn_energy` (`crates/arena/tests/rules.rs`) ; `writes_before_a_fault_stay_in_the_report` (`crates/vm/tests/memory.rs`) ; D-011 |
| 4. Isolation | `Memory = [i64; MEM_SIZE]` (`crates/vm/src/lib.rs`). `Machine::cell` (`crates/vm/src/machine.rs`) : une adresse dépilée (`LOADX`, `STOREX`) passe par `usize::try_from` puis `mem.get_mut`, sinon `OutOfBounds`. L'immédiat de `LOAD`/`STORE` est un `u8`, donc toujours dans `0..=255`. | `addresses_outside_0_255_are_out_of_bounds` et `missing_operands_underflow_before_bounds_are_checked` (`crates/vm/tests/memory.rs`) |
| 5. Absence de panic | `unsafe_code = "forbid"`, et `unwrap_used`, `expect_used`, `panic` en `deny` pour tout le workspace (`Cargo.toml`) ; en plus `indexing_slicing` et `arithmetic_side_effects` dans `pbc` et `vm` (leur `lib.rs`, D-001). `Program::load` (`crates/pbc/src/program.rs`) vérifie `MAX_CONSTANTS` et `MAX_CODE_LEN` avant d'allouer, et `read_frame` (`crates/exec/src/frame.rs`) vérifie la longueur annoncée avant d'allouer. | `load_never_panics_on_arbitrary_bytes`, `a_corrupted_byte_never_panics`, `decode_never_panics` (`crates/pbc/tests/props.rs`) ; `arbitrary_bytes_never_crash_the_server` (`crates/exec/tests/props.rs`) ; les cibles `fuzz/fuzz_targets/{load,execute,connection}.rs` et leurs campagnes (D-012) |
| 6. Coût annoncé | Une seule table de gaz, `Op::info` (`crates/pbc/src/op.rs`), lue par `Op::gas` et `Instr::gas`. `Machine::run` ne facture que `instr.gas()`, et ni `pbc` ni `vm` ne lisent l'horloge. | `gas_table_matches_b3` et `every_opcode_decodes_and_carries_its_gas` (`crates/pbc/tests/decode.rs`) ; `a_larger_budget_never_changes_a_run_that_did_not_run_out` (`crates/vm/tests/props.rs`) ; le `gas_used` comparé dans chaque cas du corpus (`crates/corpus/src/check.rs`) |

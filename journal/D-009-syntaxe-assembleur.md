# D-009 — Syntaxe de l'assembleur : marqueur explicite et désassemblage réversible

- **Identifiant :** D-009
- **Date :** 2026-10-27
- **Auteurs :** Chaimae Faris
- **Statut :** proposée
- **Commits concernés :** —

## 1. Contexte

La syntaxe d'un assembleur textuel est libre (README §2) et les mnémoniques de B.3 sont indicatifs. Il nous faut une
syntaxe pour écrire des agents sans calculer d'offsets à la main, et un désassembleur pour relire n'importe quel
`.pbc`, y compris celui d'un autre groupe à J4.

Deux règles de la spec compliquent les choses. Un saut relatif se compte depuis l'instruction suivante (B.4,
`cible = pc + taille_instruction + d`). Et un code valide peut contenir des octets qui ne se décodent pas, ou une
cible au milieu d'un opérande (A.3) : un désassembleur ne peut pas supposer que chaque octet commence une
instruction.

## 2. Options envisagées

### Option A — insérer un `JUMPDEST` devant chaque étiquette
Compromis : plus confortable, mais l'assembleur modifie le code dans le dos de l'auteur. Le désassemblage ne se
réassemble plus à l'identique, puisque chaque `JUMPDEST` relu redevient une étiquette qui en ajoute un.

### Option B — marqueur explicite
L'auteur écrit lui-même `JUMPDEST` ; l'assembleur ne fait que résoudre les adresses.

Compromis : un oubli donne `BadJump` à l'exécution, mais le binaire est exactement ce qui est écrit.

### Pour le désassemblage
Soit des étiquettes synthétiques (`L0003:`), plus lisibles, soit des déplacements bruts (`JMP -4`) annotés en
commentaire (`; 0x0004 -> 0x0000`), pour lesquels l'aller-retour est trivial.

## 3. Ce que l'assistant IA a proposé

Le marqueur explicite et des déplacements bruts dans le désassemblage. La syntaxe :

```asm
; commentaire jusqu'à la fin de la ligne
.const 1234567890123     ; ajoute une constante (index 0, 1, … dans l'ordre)
debut:                   ; étiquette = adresse de l'octet suivant
    JUMPDEST             ; une cible de saut doit porter le marqueur : l'assembleur ne l'ajoute pas
    PUSHI16 -3           ; décimal signé, ou 0x7FFF
    CONST 0              ; index de constante (u16)
    LOAD 7               ; u8
    JMP debut            ; une étiquette donne le déplacement relatif (B.4)…
    JZ +4                ; … ou un déplacement brut
    PUSHI16 @gestion     ; @étiquette = adresse absolue (pour JMPX/CALLX), qui doit tenir sur un i16
    .byte 0x43 255       ; octets bruts
```

Les mnémoniques sont insensibles à la casse, les étiquettes y sont sensibles. L'assemblage se fait en deux passes :
la première calcule l'adresse de chaque ligne (la taille d'une instruction est donnée par `Op::size`), la seconde
résout les étiquettes et encode.

Le désassemblage est un balayage linéaire : un octet qui ne se décode pas (opcode inconnu, opérande coupé par la fin
du code) sort en `.byte 0x..`. L'aller-retour `assemble(disassemble(p)) == p` tient donc pour tout programme, même
pour du code qui ne s'exécuterait pas.

L'assistant a aussi écrit le code (`crates/asm`) pour faire passer les onze tests de
`crates/asm/tests/assemble.rs` et `disassemble.rs`, eux aussi tirés de son plan. Quatre choix de détail y sont faits
sans que le plan les impose :

- une étiquette est résolue quel que soit le type d'opérande, puis la valeur obtenue est vérifiée dans la plage du
  type, exactement comme un nombre écrit à la main ;
- l'opérande est encodé selon `Op::operand`, la même table que celle du décodeur (D-003), sans passer par
  `pbc::encode`, ce qui évite un second `match` sur les 43 opcodes ;
- les nombres et les octets de `.byte` sont lus dès la passe 1 ; seules les étiquettes attendent la passe 2 ;
- `.const` et `.byte` acceptent plusieurs valeurs sur une ligne, et les directives sont insensibles à la casse,
  comme les mnémoniques.

Les sous-commandes `palestre asm`, `disasm` et `run` du plan attendent le binaire de T11 ; elles ne sont pas encore
écrites.

Ce que nous en avons vérifié : *à compléter par l'équipe.*

## 4. Ce que nous avons décidé, et pourquoi

*À rédiger par l'équipe.*

## 5. Ce qui nous ferait changer d'avis

Des agents longs où l'oubli d'un `JUMPDEST` coûte cher. On ajouterait alors un avertissement, et non une correction
silencieuse, quand une étiquette visée par un saut n'est pas suivie d'un `JUMPDEST`.

## Historique

- 2026-10-27 — proposée.

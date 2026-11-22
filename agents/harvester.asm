; Le récolteur : une seule action par tour, la première règle qui s'applique.
;   1. il porte quelque chose et son énergie est sous 50 : il mange tout ce qu'il porte ;
;   2. sa case porte une ressource : il la ramasse ;
;   3. un voisin libre porte une ressource : il y va (le premier dans l'ordre nord, est, sud, ouest) ;
;   4. il lui reste plus de 15 d'énergie : il garde son cap tant que la case devant lui est libre ; sinon il
;      en prend un nouveau, la première direction libre en partant de RAND & 3 ;
;   5. sinon, il attend.
;
; Mémoire de travail, réécrite à chaque tour : mem[0] un masque, mem[1] une direction, mem[2] un compteur.
; mem[3] garde le cap d'un tour à l'autre : sans lui, un nouveau tirage à chaque tour le fait tourner en rond.

; 1. Manger.
    SENSE 3             ; carried
    JZ ramasser
    SENSE 2             ; energy
    PUSHI16 50
    LT
    JZ ramasser
    SENSE 3
    ACT 3               ; EAT carried
    HALT

; 2. Ramasser.
ramasser:
    JUMPDEST
    SENSE 4             ; ressource ici
    JZ voisin
    PUSHI16 0
    ACT 1               ; TAKE
    HALT

; 3. Aller vers un voisin libre qui porte une ressource : ressources & ~(bloqués | agents).
voisin:
    JUMPDEST
    SENSE 9
    SENSE 8
    SENSE 13
    OR
    BNOT
    AND
    DUP
    STORE 0
    JZ errer
    PUSHI16 0
    STORE 1             ; d = 0
cherche:
    JUMPDEST
    LOAD 0
    LOAD 1
    SHR
    PUSHI16 1
    AND
    JNZ bouger          ; le bit d est à 1
    LOAD 1
    PUSHI16 1
    ADD
    STORE 1
    JMP cherche

bouger:
    JUMPDEST
    LOAD 1
    ACT 0               ; MOVE d
    HALT

; 4. Errer, si l'énergie le permet : le cap d'abord, puis un nouveau cap.
errer:
    JUMPDEST
    SENSE 2
    PUSHI16 15
    GT
    JZ attendre
    SENSE 8
    SENSE 13
    OR
    STORE 0             ; voisins bloqués
    LOAD 0
    LOAD 3
    SHR
    PUSHI16 1
    AND
    JNZ nouveau_cap     ; la case devant est bloquée
    LOAD 3
    ACT 0               ; MOVE cap
    HALT

nouveau_cap:
    JUMPDEST
    RAND
    PUSHI16 3
    AND
    STORE 1             ; d = RAND & 3
    PUSHI16 4
    STORE 2             ; quatre essais
essai:
    JUMPDEST
    LOAD 0
    LOAD 1
    SHR
    PUSHI16 1
    AND
    JZ prendre_cap      ; la direction d est libre
    LOAD 1
    PUSHI16 1
    ADD
    PUSHI16 3
    AND
    STORE 1             ; d = (d + 1) & 3
    LOAD 2
    PUSHI16 1
    SUB
    DUP
    STORE 2
    JNZ essai
    JMP attendre

prendre_cap:
    JUMPDEST
    LOAD 1
    DUP
    STORE 3             ; nouveau cap
    ACT 0               ; MOVE d
    HALT

; 5. Attendre.
attendre:
    JUMPDEST
    HALT

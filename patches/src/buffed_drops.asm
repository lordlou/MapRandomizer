lorom
arch 65816

; Make small energy drop give 10 energy:
org $86F0BB
    LDA #$000A

; Adjust drop rates of respawning enemies and Kagos:
; - Double PB drop rates of respawning enemies (Gamet, Zeb, Geega, Zebbo, Zoa, Covern)
; - Double Super drop rate of Geega, Zeb, and Kagos.
; - At the expense of nothing, small energy, and missiles drop rates
; - Shift some drop rate into large energy to compensate for loss of some small 
;                  __________________________ ; 0: Small health
;                 |     _____________________ ; 1: Big health
;                 |    |     ________________ ; 2: Missiles
;                 |    |    |     ___________ ; 3: Nothing
;                 |    |    |    |     ______ ; 4: Super missiles
;                 |    |    |    |    |     _ ; 5: Power bombs
;                 |    |    |    |    |    |
org $B4F25A : db $3C, $3C, $32, $05, $3C, $14  ; Gamet (enemy $F213)
org $B4F248 : db $14, $41, $1E, $00, $78, $14  ; Zeb (enemy $F193)   
org $B4F24E : db $14, $41, $1E, $00, $78, $14  ; Geega (enemy $F253)
org $B4F254 : db $00, $8C, $05, $00, $64, $0A  ; Zebbo (enemy $F1D3)
org $B4F260 : db $00, $64, $3C, $05, $46, $14  ; Zoa (enemy $DA7F)
org $B4F266 : db $32, $5F, $32, $00, $14, $28  ; Covern (enemy $E77F)
org $B4F26C : db $23, $5F, $3C, $05, $28, $14  ; Kago (enemy $E7FF)

; Make Power Bomb drop give 2 Power Bombs:
org $86F0D9
    LDA #$0002

; (Change we considered but didn't go forward with:)
;
;; Make Super Missile drop give 2 Super Missiles:
;org $86F0F7
;    LDA #$0002
;

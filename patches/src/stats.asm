arch 65816
lorom

incsrc "constants.asm"

!bank_80_free_space_start = $80D240
!bank_80_free_space_end = $80D340

!bank_84_free_space_start = $84F730
!bank_84_free_space_end = $84F800

!bank_85_free_space_start = $859980
!bank_85_free_space_end = $859B00


; Increment saves count when using save station
org $848D05
    jsl hook_save_station

; Increment saves count when using Ship save
org $A2AB3C
    jsl hook_ship_save

; Increment deaths count at start of death sequence
org $82DC95
    jsl hook_death : nop : nop

; Increment deaths count when escape timer runs out
org $90E0F2
    jsl hook_timeout : nop : nop

; Increment reset count on boot-up
org $808455
    jsl hook_boot

; Hook Samus forward pose entering ship
org $a2aa24
    jsl hook_samus_enter_ship : nop : nop : nop

org !bank_80_free_space_start
; end of 2nd NMI hook, determine appropriate return
area_timer:
    rep #$30
    lda !nmi_timeronly
    bne .from_timeronly
    plb
    jmp $95fc                     ; restore regs, RTI
    
.from_timeronly
    plb
    rts                           ; return to timer-only NMI

; update SRAM times and reset RAM counters
save_to_sram:
; update SRAM 32-bit stat_timer
    php
    rep #$30
    lda !nmi_counter
    and #$00FF
    clc
    adc !stat_timer
    sta !stat_timer
    bcc .no_inc
    lda !stat_timer+2
    inc
    sta !stat_timer+2

.no_inc
; update SRAM 32-bit pause/area values (contiguous)
; Y = RAM ptr, X = SRAM ptr
    ldx #$0000
    txy

.add_loop
    lda !nmi_pause, Y
    and #$00FF                    ; 8-bit update
    beq .no_inc2                  ; skip update?
    clc
    adc !stat_pause_time,x
    sta !stat_pause_time,x
    bcc .no_inc2
    lda !stat_pause_time+2,x
    inc
    sta !stat_pause_time+2,x
.no_inc2
    inx : inx : inx : inx
    iny
    cpy #$0008
    bne .add_loop

    sep #$30
    ldx #$08
    lda #$00

.clear_ram_ctrs
    sta !nmi_counter,x
    dex
    bpl .clear_ram_ctrs

    plp
    rtl

; increment area-specific RAM byte
inc_area_ram:
    lda #$00
    ldx $0998
    cpx #$1F                      ; pre-game load state?
    beq .pre_game
    cpx #$06                      ; pre-game = 0-5
    bcs .load_area
.pre_game
    lda #$06                      ; area 6 (pre-game)
    bra .update_area

.load_area
    lda $1f5b
    cmp #$06                      ; only areas 0-5 valid
    bcs .leave_inc
    cpx #$0D                      ; $0D
    bcc .update_area              ; to
    cpx #$12                      ; $11 = pause screen
    bcs .update_area
    lda #$FF                      ; pause timer

.update_area
    inc
    tax
    lda !nmi_pause,x
    inc
    sta !nmi_pause,x
.leave_inc
    rts

; 1st NMI hook
nmi_timer_hook:
    pha
    phb
    phk
    plb
    lda !nmi_timeronly
    beq .normal_nmi
    phx
    phy
    jsr inc_skipcount             ; area timer func (skip $5b8 inc)
    ply
    plx
    plb
    pla
    rti                           ; leave NMI

.normal_nmi
    plb
    pla
    phb                           ; replaced code
    phd
    pha
    jmp $958c                     ; resume NMI

enable_nmi_hook:
    stz !nmi_timeronly
    php
    sep #$20
    lda #$80
    bit $84                       ; NMI already enabled?
    beq .not_enabled
    plp
    rtl

.not_enabled
    plp
    php                           ; replaced code
    phb
    phk
    jmp $834e

disable_nmi_hook:
    inc !nmi_timeronly
    rtl

warnpc !bank_80_free_space_end

; NMI hook to check for timer-only mode
org $809589
    jmp nmi_timer_hook
    
; Disable NMI func
org $80835D
    jmp disable_nmi_hook
    
; Enable NMI func
org $80834B
    jmp enable_nmi_hook

; Boot logo NMI override
org $8b916b
    lda #$81

; Boot init CPU NMI override
org $80875d
    lda #$81

; Common boot NMI override
org $8084b3
    nop : nop : nop : nop : nop

; Start game NMI override
org $8281a7
    lda #$81

; RTA timer based on VARIA patch by total & ouiche
org $8095e5
nmi:
    ;; copy from vanilla routine without lag counter reset
    ldx #$00
    stx $05b4
    ldx $05b5
    inx
    stx $05b5
    inc $05b6
    rep #$30
    jmp .inc
warnpc $809602
org $809602 ; skip lag handling
    rep #$30
    jmp .inc
warnpc $809616
    ;; handle 32 bit counter :
org $808FA3 ;; overwrite unused routine
.inc:
    ; increment vanilla 16-bit timer (used by message boxes)
    inc $05b8
inc_skipcount:
    phb
    phk
    plb
    sep #$30
    jsr inc_area_ram
    inc !nmi_counter
    lda !nmi_counter
    cmp #$FF
    bne .leave_hook
    jsl save_to_sram
.leave_hook
    jmp area_timer

warnpc $808FC1 ;; next used routine start

!idx_ETank = #$0000
!idx_Missile = #$0001
!idx_Super = #$0002
!idx_PowerBomb = #$0003
!idx_Bombs = #$0004
!idx_Charge = #$0005
!idx_Ice = #$0006
!idx_HiJump = #$0007
!idx_SpeedBooster = #$0008
!idx_Wave = #$0009
!idx_Spazer = #$000A
!idx_SpringBall = #$000B
!idx_Varia = #$000C
!idx_Gravity = #$000D
!idx_XRayScope = #$000E
!idx_Plasma = #$000F
!idx_Grapple = #$0010
!idx_SpaceJump = #$0011
!idx_ScrewAttack = #$0012
!idx_Morph = #$0013
!idx_ReserveTank = #$0014


; hook item collection instructions:

org $84E0B6  ; ETank
    dw collect_ETank

org $84E0DB  ; Missile
    dw collect_Missile

org $84E100  ; Super
    dw collect_Super

org $84E125  ; Power Bomb
    dw collect_PowerBomb

org $84E152  ; Bombs
    dw collect_Bombs

org $84E180  ; Charge
    dw collect_Charge

org $84E1AE  ; Ice
    dw collect_Ice

org $84E1DC  ; HiJump
    dw collect_HiJump

org $84E20A  ; SpeedBooster
    dw collect_SpeedBooster

org $84E238  ; Wave
    dw collect_Wave

org $84E266  ; Spazer
    dw collect_Spazer

org $84E294  ; SpringBall
    dw collect_SpringBall

org $84E2C8  ; Varia
    dw collect_Varia

org $84E2FD  ; Gravity
    dw collect_Gravity

org $84E330  ; X-Ray
    dw collect_XRayScope

org $84E35D  ; Plasma
    dw collect_Plasma

org $84E38B  ; Grapple
    dw collect_Grapple

org $84E3B8  ; SpaceJump
    dw collect_SpaceJump

org $84E3E6  ; ScrewAttack
    dw collect_ScrewAttack

org $84E414  ; Morph
    dw collect_Morph

org $84E442  ; ReserveTank
    dw collect_ReserveTank

;;;;;;;;

org $84E472  ; ETank, orb
    dw collect_ETank

org $84E4A4  ; Missile, orb
    dw collect_Missile

org $84E4D6  ; Super, orb
    dw collect_Super

org $84E508  ; Power Bomb, orb
    dw collect_PowerBomb

org $84E542  ; Bombs, orb
    dw collect_Bombs

org $84E57D  ; Charge, orb
    dw collect_Charge

org $84E5B8  ; Ice, orb
    dw collect_Ice

org $84E5F3  ; HiJump, orb
    dw collect_HiJump

org $84E62E  ; SpeedBooster, orb
    dw collect_SpeedBooster

org $84E672  ; Wave, orb
    dw collect_Wave

org $84E6AD  ; Spazer, orb
    dw collect_Spazer

org $84E6E8  ; SpringBall, orb
    dw collect_SpringBall

org $84E725  ; Varia, orb
    dw collect_Varia

org $84E767  ; Gravity, orb
    dw collect_Gravity

org $84E7A7  ; X-Ray, orb
    dw collect_XRayScope

org $84E7E1  ; Plasma, orb
    dw collect_Plasma

org $84E81C  ; Grapple, orb
    dw collect_Grapple

org $84E856  ; SpaceJump, orb
    dw collect_SpaceJump

org $84E891  ; ScrewAttack, orb
    dw collect_ScrewAttack

org $84E8CC  ; Morph, orb
    dw collect_Morph

org $84E907  ; ReserveTank, orb
    dw collect_ReserveTank

;;;;;;;;

org $84E93D  ; ETank, shot block
    dw collect_ETank

org $84E975  ; Missile, shot block
    dw collect_Missile

org $84E9AD  ; Super, shot block
    dw collect_Super

org $84E9E5  ; Power Bomb, shot block
    dw collect_PowerBomb

org $84EA25  ; Bombs, shot block
    dw collect_Bombs

org $84EA66  ; Charge, shot block
    dw collect_Charge

org $84EAA7  ; Ice, shot block
    dw collect_Ice

org $84EAE8  ; HiJump, shot block
    dw collect_HiJump

org $84EB29  ; SpeedBooster, shot block
    dw collect_SpeedBooster

org $84EB6A  ; Wave, shot block
    dw collect_Wave

org $84EBAB  ; Spazer, shot block
    dw collect_Spazer

org $84EBEC  ; SpringBall, shot block
    dw collect_SpringBall

org $84EC2F  ; Varia, shot block
    dw collect_Varia

org $84EC77  ; Gravity, shot block
    dw collect_Gravity

org $84ECBD  ; X-Ray, shot block
    dw collect_XRayScope

org $84ECFD  ; Plasma, shot block
    dw collect_Plasma

org $84ED3E  ; Grapple, shot block
    dw collect_Grapple

org $84ED7E  ; SpaceJump, shot block
    dw collect_SpaceJump

org $84EDBF  ; ScrewAttack, shot block
    dw collect_ScrewAttack

org $84EE00  ; Morph, shot block
    dw collect_Morph

org $84EE41  ; ReserveTank, shot block
    dw collect_ReserveTank

org !bank_85_free_space_start

hook_save_station:
    jsl $868097  ; run hi-jacked Instruction
    lda !stat_saves
    inc
    sta !stat_saves
    rtl

hook_ship_save:
    jsl $818000  ; run hi-jacked Instruction
    lda !stat_saves
    inc
    sta !stat_saves
    rtl

hook_death:
    lda !stat_deaths
    inc
    sta !stat_deaths
    ; run hi-jacked instructions:
    LDX #$017E
    LDA #$0000
    rtl

hook_timeout:
    lda !stat_deaths
    inc
    sta !stat_deaths
    ; run hi-jacked instructions:
    LDX #$01FE             ;\
    LDA #$7FFF 
    rtl

hook_boot:
    lda !stat_resets
    inc
    sta !stat_resets
    JSL $8B9146 ; run hi-jacked instruction
    rtl

hook_samus_enter_ship:
    lda #$001a  ; replaced instructions
    jsl $90f084 ;
    
    lda #$000E
    jsl $808233
    bcc .end_hook ; escape?

    ; flush RTA/area timers
    jsr flush_timers

    ; capture the final time:
    lda !stat_timer
    sta !stat_final_time
    lda !stat_timer+2
    sta !stat_final_time+2    
    
    ; set area to -1 so timers stop
    lda #$FFFF
    sta $1f5b
.end_hook
    rtl

collect_item:
    phx

    ; flush RTA/area timers
    jsr flush_timers

    asl
    asl
    tax

    ; check if we have already collected one of this type of item (do not overwrite the collection time in this case):
    lda !stat_item_collection_times, x
    bne .skip
    lda !stat_item_collection_times+2, x
    bne .skip

    ; record the collection time
    lda !stat_timer
    sta !stat_item_collection_times, x
    lda !stat_timer+2
    sta !stat_item_collection_times+2, x

.skip:
    plx
    rtl

flush_timers:
    phx
    phy
    pha
    jsl save_to_sram
    pla
    ply
    plx
    rts
warnpc !bank_85_free_space_end

org !bank_84_free_space_start

collect_item_wrapper:
    jsl collect_item
    rts

collect_ETank:
    lda !idx_ETank
    jsr collect_item_wrapper
    jmp $8968

collect_Missile:
    lda !idx_Missile
    jsr collect_item_wrapper
    jmp $89A9

collect_Super:
    lda !idx_Super
    jsr collect_item_wrapper
    jmp $89D2

collect_PowerBomb:
    lda !idx_PowerBomb
    jsr collect_item_wrapper
    jmp $89FB

collect_Bombs:
    lda !idx_Bombs
    jsr collect_item_wrapper
    jmp $88F3

collect_Charge:
    lda !idx_Charge
    jsr collect_item_wrapper
    jmp $88B0

collect_Ice:
    lda !idx_Ice
    jsr collect_item_wrapper
    jmp $88B0

collect_HiJump:
    lda !idx_HiJump
    jsr collect_item_wrapper
    jmp $88F3

collect_SpeedBooster:
    lda !idx_SpeedBooster
    jsr collect_item_wrapper
    jmp $88F3

collect_Wave:
    lda !idx_Wave
    jsr collect_item_wrapper
    jmp $88B0

collect_Spazer:
    lda !idx_Spazer
    jsr collect_item_wrapper
    jmp $88B0

collect_SpringBall:
    lda !idx_SpringBall
    jsr collect_item_wrapper
    jmp $88F3

collect_Varia:
    lda !idx_Varia
    jsr collect_item_wrapper
    jmp $88F3

collect_Gravity:
    lda !idx_Gravity
    jsr collect_item_wrapper
    jmp $88F3

collect_XRayScope:
    lda !idx_XRayScope
    jsr collect_item_wrapper
    jmp $8941

collect_Plasma:
    lda !idx_Plasma
    jsr collect_item_wrapper
    jmp $88B0

collect_Grapple:
    lda !idx_Grapple
    jsr collect_item_wrapper
    jmp $891A

collect_SpaceJump:
    lda !idx_SpaceJump
    jsr collect_item_wrapper
    jmp $88F3

collect_ScrewAttack:
    lda !idx_ScrewAttack
    jsr collect_item_wrapper
    jmp $88F3

collect_Morph:
    lda !idx_Morph
    jsr collect_item_wrapper
    jmp $88F3

collect_ReserveTank:
    lda !idx_ReserveTank
    jsr collect_item_wrapper
    jmp $8986

warnpc !bank_84_free_space_end

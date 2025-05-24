arch snes.cpu
lorom

; Must match locations in patch/map_tiles.rs
!item_list_ptrs = $83B000
!item_list_sizes = $83B00C

!bank_82_freespace_start = $82FD00
!bank_82_freespace_end = $82FD80

incsrc "constants.asm"

org $82945C
    jsr transfer_pause_tilemap_hook

; patch HUD minimap drawing to copy from $703000 instead of original ROM tilemap, to pick up item dot modifications
org $90AA73
patch_hud_minimap_draw:
    lda #$0070
    sta $05
    sta $02
    sta $08
    lda #$3000
    bra .setup
warnpc $90AA8A
org $90AA8A
.setup:

org $828078
    jsl start_game_hook

org $829370
    jsl unpause_hook

org $82E498
    jsr door_transition_hook

org $85846D
    jsl message_box_hook   ; reload map after message boxes (e.g. after item collect or map station activation)
    nop

org $848AB8
    jsl door_unlocked_hook

; Free space in bank $82:
org !bank_82_freespace_start

transfer_pause_tilemap_hook:
    jsl update_pause_tilemap  ; update tilemap with collected item dots

    ; Set source tilemap to $703000 for further processing
    lda #$3000
    sta $00
    lda #$0070
    sta $02

    lda #$4000  ; run hi-jacked instruction
    rts

door_transition_hook:
    pha
    jsl update_hud_tilemap
    pla
    cmp #$0003      ; run hi-jacked instruction
    rts

warnpc !bank_82_freespace_end

; Free space in any bank:
org $83B600

start_game_hook:
    sta $7EC380,x  ; run hi-jacked instruction
    jsl update_hud_tilemap
    rtl

unpause_hook:
    jsl $80A149  ; run hi-jacked instruction
    jsl update_hud_tilemap
    rtl

message_box_hook:
    jsl update_hud_tilemap
    
    ; run hi-jacked instructions:
    sep #$20
    lda $1C1F
    rtl

door_unlocked_hook:
    sta $7ED8B0,x ; run hi-jacked instruction (store new door unlocked bit)
    jsl update_hud_tilemap
    rtl

update_hud_tilemap:
    php
    phx
    phy

    jsl $85A280    ; load_bg3_map_tilemap_wrapper (map_area.asm)

    ldx $07BB      ; x <- room state pointer
    lda $8F0010,x
    tax            ; x <- extra room data pointer
    lda $B80007,x
    tax            ; x <- pointer to dynamic tile data (list of 6-byte records) in bank $E4

    ; $03 <- $703000  (destination tilemap)
    lda #$3000
    sta $03
    lda #$0070
    sta $05

    lda $E40000,x
    sta $06        ; $06 <- loop counter: size of list
    beq .done      ; if there are no dynamic tiles for this room, then we're done
    inx : inx

.item_loop:
    lda #$0000
    sep #$20
    lda $E40000, x
    phx
    tax
    lda $7ED870, x
    plx
    and $E40001, x
    bne .skip  ; item is collected, so skip overwriting tile

    rep #$20
    lda $E40002, x  ; A <- tilemap offset
    tay
    lda $E40004, x  ; A <- tilemap word
    sta [$03], y

.skip:
    rep #$20
    inx : inx : inx : inx : inx : inx
    dec $06
    bne .item_loop

.done:
    rep #$20
    lda #$0000
    sta !last_samus_map_y  ; reset Samus map Y coordinate, to trigger minimap to update
    ply
    plx
    plp
    rtl

update_pause_tilemap:
    php
    phx
    phy
    rep #$30

    ; $00 <- long pointer to original map tilemap in ROM, for current area
    lda $1F5B  ; current map area
    asl A
    clc
    adc $1F5B
    tax
    lda $82964C,x
    sta $02
    lda $82964A,x
    sta $00

    ; Copy tilemap from [$00] (original map tilemap in ROM) to $703000 (in SRAM, which we're just using as extra RAM)
    ldx #$0800  ; loop counter
    ldy #$0000  ; offset

    lda #$3000
    sta $03
    lda #$0070
    sta $05

.copy_loop:
    lda [$00], y
    sta [$03], y
    iny
    iny
    dex
    bne .copy_loop

    lda $1F5B
    asl
    tax
    lda !item_list_sizes, x
    beq .done  ; If there are no items in this area, then we're done.
    sta $06    ; $06 <- loop counter
    lda !item_list_ptrs, x
    tax  ; X <- item data offset

.item_loop:
    lda #$0000
    sep #$20
    lda $830000, x
    phx
    tax
    lda $7ED870, x
    plx
    and $830001, x
    bne .skip  ; item is collected, so skip overwriting tile

    rep #$20
    lda $830002, x  ; A <- tilemap offset
    tay
    lda $830004, x  ; A <- tilemap word
    sta [$03], y

.skip:
    rep #$20
    inx : inx : inx : inx : inx : inx
    dec $06
    bne .item_loop

.done:
    ply
    plx
    plp
    rtl

warnpc $83B700

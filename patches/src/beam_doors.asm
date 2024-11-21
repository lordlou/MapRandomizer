lorom

; We have one beam door PLM type for each of the 4 directions. Different beam types 
; (Charge, Ice, etc.) share the same PLM. We constrain to having at most one beam door per room, 
; allowing us to reserve only enough tiles in VRAM for one beam type (and orientation) at a time, 
; which get loaded when entering the room. The beam type is also stored in $1F76 when entering
; the room, used to determine the collision reaction for opening the door.
;
; Unpause hook to reload beam tiles is in hazard_markers.asm (to avoid needing an additional hook)

!bank_84_free_space_start = $84FCC0
!bank_84_free_space_end = $858000

; Low RAM location containing the beam type of the beam door in the room
; (0=Charge, 1=Ice, 2=Wave, 3=Spazer, 4=Plasma)
!beam_type = $1F76

!beam_tilemap_size = #$0020
!beam_tilemap_start = $7EA720

; Bank containing beam tilemaps and graphics
!gfx_src_bank = #$00EA
; Low RAM location containing base address in bank EA. This varies depending on the
; type of beam door in the room.
!gfx_src_base_addr = $1F78  

; 8-tiles GFX source offsets, the full door height including the brackets:
; These offsets are added to the base offset in !gfx_src_base_addr, in bank EA
!gfx_initial = $0020
!gfx_opening_1 = $0120
!gfx_opening_2 = $0220
!gfx_opening_3 = $0320
; 4-tile GFX, only the beam-specific GFX in the center:
!gfx_idle_0 = $0420
!gfx_idle_1 = $04A0
!gfx_idle_2 = $0520
!gfx_idle_3 = $05A0


org !bank_84_free_space_start

; These PLMs definitions must go here first, as they are referenced in patch.rs
beam_door_right_plm:
    dw $C7B1, right_inst, right_inst_closing

beam_door_left_plm:
    dw $C7B1, left_inst, left_inst_closing

beam_door_down_plm:
    dw $C7B1, down_inst, down_inst_closing

beam_door_up_plm:
    dw $C7B1, up_inst, up_inst_closing

warnpc $84FCD8
org $84FCD8

; The address here must match what is used in patch.rs
; This gets called during the transition while entering a room with a beam door.
; Input:
;   A = beam type (0=Charge, 1=Ice, 2=Wave, 3=Spazer, 4=Plasma)
;   X = base address pointing to tilemap and graphics to load (from bank EA)
load_beam_tiles:
    phb
    sta $1F76                ; store the beam type
    stx $1F78                ; store the base address

    pea $7E7E
    plb
    plb

    ; Load the tilemap (16x16) for the beam door. This remains unchanged while in the room:
    ldy #$0000
.loop:
    lda $EA0000, x
    sta.w !beam_tilemap_start, y
    inx
    inx
    iny
    iny
    cpy !beam_tilemap_size
    bne .loop

    plb
    rtl

print pc
set_timer:
    LDA $0000,y
    STA $7EDE1C,x   ; PLM instruction timer = [Y]
    INY
    INY
    TYA
    STA $1D27,x     ; Set instruction pointer to the following instruction
    PLA             ; Return from processing PLM, instead of looping to next instruction
    RTS


update_beam_gfx:
    ; queue transfer of 8 tiles (256 bytes) to VRAM:
    PHX
    LDX $0330       ; get VRAM write table stack pointer
    LDA $00         ; size = [$00]
    STA $D0,x 
    INX
    INX
    LDA $0000,y
    CLC
    ADC !gfx_src_base_addr
    STA $D0,x       ; source address = Y
    INX
    INX
    LDA !gfx_src_bank   ; source bank = $EA
    STA $D0,x
    INX
    LDA $02         ; VRAM destination = [$02]
    STA $D0,x
    INX
    INX
    STX $0330       ; update VRAM write table stack pointer
    INY
    INY
    PLX
    RTS


; input: [Y] = source address offset for GFX data (8 tiles = 256 bytes) in bank EA
print pc
update_beam_gfx_8_tile:
    ; queue transfer of 8 tiles (256 bytes) to VRAM:
    LDA #$0100      ; size = #$0100 bytes
    STA $00
    LDA #$2600      ; VRAM destination = $2600
    STA $02
    BRA update_beam_gfx

; input: [Y] = source address offset for GFX data (4 tiles = 128 bytes) in bank EA
print pc
update_beam_gfx_4_tile:
    ; queue transfer of 4 tiles (128 bytes) to VRAM:
    LDA #$0080      ; size = #$0080 bytes
    STA $00
    LDA #$2620      ; VRAM destination = $2620
    STA $02 
    BRA update_beam_gfx

animate_loop_inst:
    dw set_timer, $0008
    dw update_beam_gfx_4_tile, !gfx_idle_1
    dw set_timer, $0008
    dw update_beam_gfx_4_tile, !gfx_idle_2
    dw set_timer, $0008
    dw update_beam_gfx_4_tile, !gfx_idle_3
    dw set_timer, $0008
    dw update_beam_gfx_4_tile, !gfx_idle_0
    dw $8724, animate_loop_inst

right_inst_closing:
    dw $0002, $A677
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0002, right_draw_half_open
    dw $8C19                 ; Queue sound 8, sound library 3, max queued sounds allowed = 6 (door closed)
    db $08
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw $0002, right_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw set_timer, $0002
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0001, right_draw_shootable

right_inst:
    dw $8A72, $C4B1          ; Go to $C4B1 if the room argument door is set
    dw $8A24, .open          ; Link instruction = .open
    dw $86C1, check_shot     ; Pre-instruction = go to link instruction if shot with correct beam
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0001, right_draw_shootable
    dw $8724, animate_loop_inst   ; Go to animate_loop_inst
.open:
    dw $8A91                 ; Set room argument to remember that door is unlocked when next entering
    db $01
    dw .unlocked
.unlocked:
    dw $8C19                 ; Queue sound 7, sound library 3, max queued sounds allowed = 6 (door opened)
    db $07        
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw $0004, right_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw set_timer, $0004
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0004, right_draw_half_open
    dw $0001, $A677
    dw $86BC                 ; Delete

left_inst_closing:
    dw $0002, $A683
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0002, left_draw_half_open
    dw $8C19                 ; Queue sound 8, sound library 3, max queued sounds allowed = 6 (door closed)
    db $08
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw $0002, left_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw set_timer, $0002
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0001, left_draw_shootable

left_inst:
    dw $8A72, $C4E2          ; Go to $C4E2 if the room argument door is set
    dw $8A24, .open          ; Link instruction = .open
    dw $86C1, check_shot     ; Pre-instruction = go to link instruction if shot with correct beam
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0001, left_draw_shootable
    dw $8724, animate_loop_inst   ; Go to animate_loop_inst
.open:
    dw $8A91                 ; Set room argument to remember that door is unlocked when next entering
    db $01
    dw .unlocked
.unlocked:
    dw $8C19                 ; Queue sound 7, sound library 3, max queued sounds allowed = 6 (door opened)
    db $07        
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw $0004, left_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw set_timer, $0004
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0004, left_draw_half_open
    dw $0001, $A683
    dw $86BC                 ; Delete

up_inst_closing:
    dw $0002, $A69B
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0002, up_draw_half_open
    dw $8C19                 ; Queue sound 8, sound library 3, max queued sounds allowed = 6 (door closed)
    db $08
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw $0002, up_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw set_timer, $0002
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0001, up_draw_shootable

up_inst:
    dw $8A72, $C544          ; Go to $C544 if the room argument door is set
    dw $8A24, .open          ; Link instruction = .open
    dw $86C1, check_shot     ; Pre-instruction = go to link instruction if shot with correct beam
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0002, up_draw_shootable
    dw $0001, up_draw_shootable   ; Draw a second time to ensure it shows correctly in Red Brinstar Elevator Room and Forgotten Highway
    dw $8724, animate_loop_inst   ; Go to animate_loop_inst
.open:
    dw $8A91                 ; Set room argument to remember that door is unlocked when next entering
    db $01
    dw .unlocked
.unlocked:
    dw $8C19                 ; Queue sound 7, sound library 3, max queued sounds allowed = 6 (door opened)
    db $07        
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw $0004, up_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw set_timer, $0004
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0004, up_draw_half_open
    dw $0001, $A69B
    dw $86BC                 ; Delete

down_inst_closing:
    dw $0002, $A68F
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0002, down_draw_half_open
    dw $8C19                 ; Queue sound 8, sound library 3, max queued sounds allowed = 6 (door closed)
    db $08
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw $0002, down_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw set_timer, $0002
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0001, down_draw_shootable

down_inst:
    dw $8A72, $C513          ; Go to $C513 if the room argument door is set
    dw $8A24, .open          ; Link instruction = .open
    dw $86C1, check_shot     ; Pre-instruction = go to link instruction if shot with correct beam
    dw update_beam_gfx_8_tile, !gfx_initial
    dw $0001, down_draw_shootable
    dw $8724, animate_loop_inst   ; Go to animate_loop_inst
.open:
    dw $8A91                 ; Set room argument to remember that door is unlocked when next entering
    db $01
    dw .unlocked
.unlocked:
    dw $8C19                 ; Queue sound 7, sound library 3, max queued sounds allowed = 6 (door opened)
    db $07        
    dw update_beam_gfx_8_tile, !gfx_opening_1
    dw $0004, down_draw_solid
    dw update_beam_gfx_8_tile, !gfx_opening_2
    dw set_timer, $0004
    dw update_beam_gfx_8_tile, !gfx_opening_3
    dw $0004, down_draw_half_open
    dw $0001, $A68F
    dw $86BC                 ; Delete

print pc 
check_shot:
    lda $1D77,x
    beq .done      ; Return if not shot

    phx    
    lda !beam_type
    asl
    tax
    lda beam_mask,x
    sta $0E        ; $0E = mask associated with beam type
    plx

    lda $1d77,x
    and $0E
    bne .hit       ; check if projectile beam matches door type
    jsr $F900      ; check for hyper shot in escape (`escape_hyper_door_check` in escape.asm)
    bcs .hit

    lda #$0057     ;
    jsl $8090CB    ;} Queue sound 57h, sound library 2, max queued sounds allowed = 6 (shot door/gate with dud shot)
    stz $1D77,x    ; clear PLM shot status
    rts

print "hit: ", pc
.hit:
    lda $7EDEBC,x          ;\
    sta $1D27,x            ;} PLM instruction list pointer = [PLM link instruction]
    lda #$0001             ;\
    sta $7EDE1C,x          ;} PLM instruction timer = 1
    stz $1D77,x            ; clear PLM shot status

.done:
    rts

beam_mask:
    dw $0010, $0002, $0001, $0004, $0008   ; Charge, Ice, Wave, Spazer, Plasma

right_draw_shootable:
    dw $8004, $C4E4, $D4E5, $D4E6, $D4E7
    dw $0000

right_draw_solid:
    dw $8004, $84E4, $84E5, $84E6, $84E7
    dw $0000

right_draw_half_open:
    dw $8004, $84E4, $04E5, $04E6, $84E7
    dw $0000

left_draw_shootable:
    dw $8004, $C0E4, $D0E5, $D0E6, $D0E7   ; shootable blocks
    dw $0000

left_draw_solid:
    dw $8004, $80E4, $80E5, $80E6, $80E7   ; solid blocks
    dw $0000

left_draw_half_open:
    dw $8004, $80E4, $00E5, $00E6, $80E7   ; inner blocks air, outer blocks solid
    dw $0000

up_draw_shootable:
    dw $0004, $C0E4, $50E5, $50E6, $50E7
    dw $0000

up_draw_solid:
    dw $0004, $80E4, $80E5, $80E6, $80E7
    dw $0000

up_draw_half_open:
    dw $0004, $80E4, $00E5, $00E6, $80E7
    dw $0000

down_draw_shootable:
    dw $0004, $C8E4, $58E5, $58E6, $58E7
    dw $0000

down_draw_solid:
    dw $0004, $88E4, $88E5, $88E6, $88E7
    dw $0000

down_draw_half_open:
    dw $0004, $88E4, $08E5, $08E6, $88E7
    dw $0000

print pc 
warnpc !bank_84_free_space_end
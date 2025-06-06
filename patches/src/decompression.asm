; From https://github.com/theonlydude/RandomMetroidSolver/blob/master/patches/common/src/decompression.asm
;$30FF-$3265

;;; Decompression optimization by Kejardon, with fixes by PJBoy and Maddo

;Compression format: One byte(XXX YYYYY) or two byte (111 XXX YY-YYYYYYYY) headers
;XXX = instruction, YYYYYYYYYY = counter

;Modified by Tundain (20/12/2023: DMY)

;changes:
;-when checking for overflow, the branch is now taken whenever there is overflow, rather than when there's not
;    because a non-taken branch is 1 cycle cheaper, and this happens more often
;    this leads to a bit more code, but i believe it's worth it (there's some space left before hitting the VRAM decomp routine
;-direct copy has been moved in front of the main loop, because that way a JMP doesn't need to get taken from there.
;-word fill has been modified to go straight into 16-bit mode, to avoid having to load a word whilst in 8-bit mode
;-Because storing happens more often than loading during decompression, DB is now set to the destination bank.
;    this means loading now uses indirect long, and storing indirect short
;    updating source bank is also simpler now


;doesn't change that much, but it is faster!


;Overhauled by Tundain (26/12/2024: DMY)
;Huge overhaul this time for significant performance improvement!
; I changed the way it writes data to use the WRAM I/O port ($2180)
; I also made the decompression commands use DMA transfers where applicable
; I also changed the DP during decompression to $4300 and moved all temporary variables usage to over there, since DMA channel RAM is apparently faster than WRAM.
; I also undid some changes in the previous version because they became impractical
; and a bunch of other small tweaks

;Also optimizes the VRAM decompression routine!

;unfortunately now requires a bit of freespace in bank $80 
;if any other patch you're using conflicts, feel free to repoint

; compatibility note for SNES Mini: DMA registers $43xA are prone to corruption, should not be used for temp storage

incsrc "constants.asm"

!bank_7e_free_space_start = $7EF4B0
!bank_7e_free_space_end = $7EF4E0
!dma_register_backup = !bank_7e_free_space_start

!bank_80_free_space_start = $80E2A0
!bank_80_free_space_end = $80E3C0

!bank_89_free_space_start = $89B0C0
!bank_89_free_space_end = $89B110

;also uses one unused RAM variable
;if another patch also uses this RAM variable, find yourself a different one
!DecompFlag = $9B

lorom

macro setup()
 ;DP RAM is relocated to the DMA channels because faster cycles
 ;channel 2 is used to write a repeating word/byte
 ;direction is B->A, so the A address ($23-$24) becomes the destination address
 ;channel 3 is for direct copy, so A->b, with B being the WRAM port
 ;other channels are used as temp variables
setup:
    SEP #$20
    LDA #$01 : STA !DecompFlag

    STZ $85 : STZ $420C  ; disable HDMA channels
    LDX $47  ; X <- source address (compressed data)

    ; Direct Page register D <- #$4300
    TDC : LDA #$43 : XBA : TCD

    ; save 16-bit register values $43xx
    REP #$20
    LDA $08 : STA !dma_register_backup+$00
    LDA $18 : STA !dma_register_backup+$02
    LDA $20 : STA !dma_register_backup+$04
    LDA $22 : STA !dma_register_backup+$06
    LDA $24 : STA !dma_register_backup+$08
    LDA $30 : STA !dma_register_backup+$0A
    LDA $32 : STA !dma_register_backup+$0C
    LDA $34 : STA !dma_register_backup+$0E
    LDA $42 : STA !dma_register_backup+$10
    LDA $50 : STA !dma_register_backup+$12
    LDA $52 : STA !dma_register_backup+$14
    LDA $54 : STA !dma_register_backup+$16
    LDA $62 : STA !dma_register_backup+$18
    LDA $64 : STA !dma_register_backup+$1A
    LDA $70 : STA !dma_register_backup+$1C
    LDA $72 : STA !dma_register_backup+$1E
    LDA $74 : STA !dma_register_backup+$20

    SEP #$20

    ;save HDMA bank for channels 2/3 (mini fix)
    LDA $27 : STA !dma_register_backup+$22
    LDA $37 : STA !dma_register_backup+$23

    LDA $004C : STA $2181 : STA $42 : STA $22
    LDA $004D : STA $2182 : STA $43 : STA $23
    LDA $004E : STA $2183 : STA $64 : STA $24 : STA $73 : STA $74;$73/$74 are the argumants for the MVN
    LDA $0049 : STA $34
    ;channel 2/3 settings
    LDA #$34 : STA $21
    LDA #$81 : STA $20
    LDA #$80 : STA $31
    LDA #$00 : STA $30
    
    LDA #$54 : STA $72;write the MVN
    LDA #$60 : STA $75;write the RTS for the MVN
    LDA $0049 : PHA : PLB
    CLC
endmacro


;;; hooks

org $8095A7
    JMP checkIfInLoading

; Change the top-of-HUD IRQ to run earlier, starting during vblank (after NMI):
; Normally this IRQ fires during scanline 0, but if decompression is doing a large
; DMA transfer, then the IRQ could be delayed, resulting in artifacts showing at the
; top of the HUD. By running it earlier the IRQ will usually finish before vblank ends,
; and always before scanline 8 where a visible artifact could occur.
; For game states when decompression may be happening, we set it to run on scanline 
; $FF (255). If NMI is somehow still in progress, the IRQ should fire immediately
; after NMI returns, or after the completion of any DMA transfer that was
; interrupted by NMI. We saw compatibility issues on 3DS (Snes9x 1.51), which is
; why we set it to a relatively late scanline so that NMI should no longer still be
; in progress.
;
; For main gameplay states (where decompression is not happening), we adjust the timing
; by a smaller amount (37 dots), to account for the extra few cycles spent saving and
; setting the Direct Page register.
;
org $8096CF : LDX #$0073  ; main gameplay (h-counter target = $73 = 115)
org $809713 : LDY #$00FF  ; start of door transition (v-counter target = $FF = 255)
org $809754 : LDX #$0073  ; Draygon's Room (h-counter target = $73 = 115)
org $8097B4 : LDY #$00FF  ; vertical door transition (v-counter target = $FF = 255)
org $80981D : LDY #$00FF  ; horizontal door transition (v-counter target = $FF = 255)

; Change the bottom-of-HUD IRQ to run slightly earlier (37 dots), to account for the extra few
; cycles spent saving and setting the Direct Page register:
; It is also possible for this IRQ to be delayed due to a large DMA transfer during
; decompression, but if that happens, it only results in some black scanlines below
; the HUD, which shouldn't be too noticeable since the room is already faded to black.
org $8096A5 : LDX #$0073  ; main gameplay (h-counter target = $73 = 115)
org $8096ED : LDX #$0073  ; start of door transition (h-counter target = $73 = 115)
org $80972F : LDX #$0073  ; Draygon's Room (h-counter target = $73 = 115)
org $80976D : LDX #$0073  ; vertical door transition (h-counter target = $73 = 115)
org $8097D6 : LDX #$0073  ; horizontal door transition (h-counter target = $73 = 115)

;some hijacks
org $809876
    JMP setDp
return:

org $809883
    JMP ResetDp
return2:

org $80A15B
    JSR unpause_hook : NOP

org $82E7A8
    JSL VRAMdecomp

org $82E7BB
    JSL VRAMdecomp

;;; new decompression func

org $80B0FF
    LDA $02, S
    STA $45
    LDA $01, S
    STA $44        ;Address of target address for data into $44
    CLC
    ADC #$0003
    STA $01, S    ;Skip target address for RTL
    LDY #$0001
    LDA [$44], Y
    STA $4C
    INY
    LDA [$44], Y
    STA $4D

warnpc $80B119

org $80B119; : Later JSL start if 4C (target address) is specified
{;regular decompression
namespace DEFAULT
Start:
    PHP : PHB : PHD
    %setup()
    BRA NextByte

End:
    JMP cleanup

    Option0:;this one is here so one of the commands doesn't need a JMP to go back to the start (ugly, but saves those extra 3 cycles)
    REP #$20
    STY $35;size
    STX $32;address
    TXA : CLC : ADC $18 : ORA #$8000 : TAX : BCC +
    AND #$7FFF : STA $70;size of data in next bank
    TYA : SEC : SBC $70 : STA $35;size of data in current bank
    SEP #$20
    LDA #$08 : STA $80420B
    LDA $70 : STA $35
    LDA #$80 : STA $33 : INC $34
    LDA $34 : PHA : PLB
    LDA $70 : BEQ start
    +
    SEP #$20
    LDA #$08 : STA $80420B
start:
    REP #$20
    LDA $22 : CLC : ADC $18 : STA $22;update destination address
    SEP #$20
NextByte:
    LDA $0000,x : INX : BNE +
    JSR IncrementBank
    +
    STA $08
    CMP #$FF : BEQ End
    CMP #$E0 : BCC ++
    ASL #3 : AND #$E0 : STA $54
    LDA $08 : AND #$03 : XBA
    LDA $0000,x : INX : BNE +
    JSR IncrementBank
    +
    BRA +++
++
    AND #$E0 : STA $54
    TDC : XBA
    LDA $08 : AND #$1F
+++
    TAY : INY : STY $18
    LDA $54
    BMI Option4567
    BNE +
    JMP Option0 : +
    CMP #$20 : BEQ BRANCH_THETA
    CMP #$40 : BEQ BRANCH_IOTA
;X = 3: Store an ascending number (starting with the value of the next byte) Y times.
    LDA $0000,x : INX : BNE +
    JSR IncrementBank
    +
-
    STA $802180 : INC : DEY
    BNE -
    JMP start
BRANCH_THETA:
    ;X = 1: Copy the next byte Y times.
    LDA $0000,x : XBA : LDA $0000,x : INX : BNE +
    JSR IncrementBank
    +
-    
    STA $80211B
    edgecase1:
    STA $80211B
    BRA transfer
super_rare_edgecase3:
    XBA : BRA rare_return
BRANCH_IOTA:
    LDA $0000,x : XBA : INX : BNE +
    JSR IncrementBank
    +
    LDA $0000,x : XBA : INX : BNE +
    JSR IncrementBank
    +
    ;X = 2: Copy the next two bytes, one at a time, for the next Y bytes.
    rare_return:
    STA $80211B
    edgecase2:
    XBA
    edgecase3:
    STA $80211B;place word to repeatedly copy in multiplication register
    transfer:
    STY $25;set size
    LDA #$01 : STA $80211C
    LDA #$04 : STA $80420B;initiate DMA
    REP #$20
    LDA $22 : STA $802181;update destination address
    SEP #$20
++
    JMP NextByte;DMA autoincrements, so we can skip updating address section
Option4567:
    CMP #$C0
    AND #$20    ;X = 4: Copy Y bytes starting from a given address in the decompressed data.
    STA $52     ;X = 5: Copy and invert (EOR #$FF) Y bytes starting from a given address in the decompressed data.
    BCS +++     ;X = 6 or 7 branch
    LDA $0000,x : XBA : INX : BNE +
    JSR IncrementBank
    +
    LDA $0000,x : XBA : INX : BNE +
    JSR IncrementBank
    +
    REP #$21
    ADC $42 : STA $62;add starting offset(where we're decompressing to)
--
    SEP #$20
    LDA $52
    BNE +    ;Inverted
-
    ;the non-inverted dictionnary copy uses MVN for speed
    ;it's faster as long as the data to copy is 4+ bytes long
    STX $50 
    LDX $62 
    REP #$20
    TYA : DEC   
    LDY $22 
    JSR $4372
    STY $22 : TYA : STA $802181 
    SEP #$20
    LDA $34 : PHA : PLB
    LDX $50 
    JMP NextByte
+
-
    LDA [$62];
    EOR #$FF
    STA $802180
    INC $62 : BNE ++
    INC $63
    ++
    DEY

    BNE -
    JMP start
+++
    ;X = 6: Copy Y bytes starting from a given number of bytes ago in the decompressed data.
    ;X = 7: Copy and invert (EOR #$FF) Y bytes starting from a given number of bytes ago in the decompressed data.

    TDC : XBA

    LDA $0000,x : INX : BNE +
    JSR IncrementBank
    +

    REP #$20
    STA $62
    LDA $22 : SBC $62 : STA $62

    BRA --
namespace off    
}

IncrementBank:
    LDX #$8000
    PHA : PHB : PLA : INC : STA $34 : PHA : PLB : PLA
    RTS

{;decompression to VRAM
VRAMdecomp:
print "vram: $",pc
    ; $47-$49: source address
    ; $2116: destination VRAM address
    php

    ; decompress to RAM at $7e3000:
    ; during start game or reload, this space is free.
    ; but during unpause, it may overwrite stuff, which would need to be saved and restored.
    lda #$0000
    sta $4C
    lda #$7E30
    sta $4D
    jsl DEFAULT_Start

    ; DMA the decompressed output from RAM to VRAM:
    lda #$1801
    sta $4310  ; DMA control: DMA transfer from CPU to VRAM, incrementing CPU address    
    lda #$3000 ; source address
    sta $4312
    lda #$007E ; source bank = $7E
    sta $4314
    txa
    sec
    sbc #$3000
    sta $4315  ; transfer size = final destination address at end of decompression (X) - $2000
    lda #$0002
    sta $420B  ; perform DMA transfer on channel 1
    
    plp
    rtl

}

; We don't care if we overwrite some of the "failed NTSC/PAL check" tilemap.
print PC
warnpc $80BC37

org !bank_80_free_space_start

cleanup:
    LDX $22  ; X <- final destination address (used in decompression to VRAM, to determine output size)
    PHX

    ;restore some DMA registers that could have been overwritten
    REP #$20
    PHB
    PHK
    PLB
    LDX #$0000
    TXY
.restore_lp
    LDA restore_table, Y
    AND #$00FF
    BEQ skip_tbl
    PHY
    TAY
    LDA !dma_register_backup, X
    STA $4300,Y
    PLY
    INY
    INX : INX
    bra .restore_lp

restore_table: ; null-terminated
    db $08, $18, $20, $22, $24, $30, $32, $34, $42, $50, $52, $54, $62, $64, $70, $72, $74, $00

skip_tbl:
    PLB
    PLX
    SEP #$20

    LDA !dma_register_backup+$22 : STA $27
    LDA !dma_register_backup+$23 : STA $37

    PLD
    STZ !DecompFlag
    PLB : PLP : RTL


;if decompression is busy, don't handle HDMA queue (this would overwrite the DMA channels we're using)
checkIfInLoading:
    JSR $91EE
    LDX !DecompFlag : BNE .check_stack
    JMP $95AA

;hyper-rare edge case where NMI interrupts a write to a write-twice register
.check_stack
    LDA $0D,s
    AND #$00FF
    CMP #$0080 ; confirm bank 80
    BNE +++
    LDA $0B,s
    CMP.w #DEFAULT_edgecase1 : BEQ +
    CMP.w #DEFAULT_edgecase2 : BEQ +
    CMP.w #DEFAULT_edgecase3 : BEQ ++
    BRA +++
+
    LDA.w #DEFAULT_rare_return : STA $0B,s : BRA +++
++
    LDA.w #DEFAULT_super_rare_edgecase3 : STA $0B,s
+++
    JMP $95C0

;IRQ also preserves DP
setDp:
    PHD
    REP #$20 : LDA #$0000 : TCD
    LDA $4211 : JMP return
ResetDp:
    PLD : STX $4207 : JMP return2
print "unpause_hook: ", pc
unpause_hook:
    ; measure the current value of frame counter
    lda !nmi_counter
    sta $0E

    phb

    ; backup RAM in the range $7E5000-$7E5400 to SRAM $703000
    ldx #$5000
    ldy #$3000
    lda #$03FF
    mvn $70, $7e
 
    ; backup RAM in the range $7E7000-$7EB000 to SRAM $703400
    ldx #$7000
    ldy #$3400
    lda #$3FFF
    mvn $70, $7e
    
    plb

    jsl $82E783  ; run hi-jacked instruction (reload tileset graphics)
    phb

    ; restore RAM in the range $7E5000-$7E5400 from SRAM $703000
    ldx #$3000
    ldy #$5000
    lda #$03FF
    mvn $7e, $70

    ; restore RAM in the range $7E7000-$7EB000 from SRAM $703400
    ldx #$3400
    ldy #$7000
    lda #$3FFF
    mvn $7e, $70

    plb

    ; compare current value of frame counter with previously measured value,
    ; to determine how many frames to lag (to make the unpause black screen a consistent length).
    lda !nmi_counter
    sec
    sbc $0E
    and #$00FF
    sta $0E
    lda !unpause_black_screen_lag_frames
    sbc $0E
    bmi .done

    ; wait the for the given amount of frames to elapse
    tax
.loop:
    beq .done
    lda !nmi_counter
.wait_frame:
    cmp !nmi_counter  ; wait for frame counter to change
    beq .wait_frame
    lda !nmi_counter
    dex
    bra .loop
.done:
    rts

warnpc !bank_80_free_space_end

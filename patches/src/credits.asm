; Based on VARIA timer/stats/credits by total & ouiche
; Adapted for Map Rando by Maddo.

arch 65816
lorom

incsrc "constants.asm"

!bank_8b_free_space_start = $8bf770
!bank_8b_free_space_end = $8bf900
!bank_ce_free_space_start = $ceb240  ; must match address in patch.rs
!bank_ce_free_space_end = $ced000
!bank_df_free_space_start = $dfd4df
!bank_df_free_space_end = $dfd91b
!stats_table_address = $dfdf80  ; must match address in patch.rs
!scroll_speed = $7fffe8

!credits_tilemap_offset = $0034

;; Defines for the script and credits data
!speed = set_scroll
!set = $9a17
!delay = $9a0d
!draw = $0000
!end = $f6fe, $99fe
!blank = $1fc0
!row = $0040
!pink = "table tables/pink.tbl,rtl"
!yellow = "table tables/yellow.tbl,rtl"
!cyan = "table tables/cyan.tbl,rtl"
!blue = "table tables/blue.tbl,rtl"
!green = "table tables/green.tbl,rtl"
!orange = "table tables/orange.tbl,rtl"
!purple = "table tables/purple.tbl,rtl"
!big = "table tables/big.tbl,rtl"


;; Hijack the original credits code to read the script from bank $DF

org $8b9976
    jml scroll

org $8b999b
    jml patch1

org $8b99e5
    jml patch2

org $8b9a08
    jml patch3

org $8b9a19
    jml patch4

;; Hijack after decompression of regular credits tilemaps
org $8be0d1
    jsl copy

org !bank_8b_free_space_start

;; set scroll speed routine (!speed instruction in credits script)
set_scroll:
    rep #$30
    phb : pea $df00 : plb : plb
    lda $0000, y
    sta !scroll_speed
    iny
    iny
    plb
    rts

scroll:
    inc $1995
    lda $1995
    cmp !scroll_speed
    beq +
    lda $1997
    jml $8b9989
+
    stz $1995
    inc $1997
    lda $1997
    jml $8b9989


patch1:
    phb : pea $df00 : plb : plb
    lda $0000, y
    bpl +
    plb
    jml $8b99a0
+
    plb
    jml $8b99aa

patch2:
    sta $0014
    phb : pea $df00 : plb : plb
    lda $0002, y
    plb
    jml $8b99eb

patch3:
    phb : pea $df00 : plb : plb
    lda $0000, y

    tay
    plb
    jml $8b9a0c

patch4:
    phb : pea $df00 : plb : plb
    lda $0000, y
    plb
    sta $19fb
    jml $8b9a1f

;; Copy custom credits tilemap data from ROM to $7f2000,x
copy:
    pha
    phx
    ldx #$0000
-
    lda.l credits, x
    cmp #$0000
    beq +
    sta $7f2000, x
    inx
    inx
    jmp -
+

;    ldx #$0000
;-
;    lda.l itemlocations, x
;.n
;    rep 4 : nop
;    cmp #$0000
;    beq +
;    sta $7fa000, x
;    inx
;    inx
;    jmp -
;+
;
    jsl write_stats

    lda #$0002
    sta !scroll_speed
    plx
    pla
    jsl $8b95ce
    rtl

warnpc !bank_8b_free_space_end

org !bank_df_free_space_start

;; Draw full time as hh:mm:ss
;; $18: Long pointer to 32-bit time in frames
draw_full_time:
    phx
    phb
    lda [$18]
    sta $16
    inc $18
    inc $18
    lda [$18]
    sta $14

    bne .non_zero
    lda $16
    bne .non_zero
    plb
    plx
    rtl

.non_zero:
    ; draw colons for time separators
    lda #$005A  ; space
    sta !credits_tilemap_offset-2, y
    sta !credits_tilemap_offset-2+!row, y
    sta !credits_tilemap_offset+4, y
    sta !credits_tilemap_offset+4+!row, y

    lda #$003c
    sta $12
    lda #$ffff
    sta $1a
    jsl div32 ;; frames in $14, rest in $16
    ; Skip drawing frames:
;    rep 6 : iny ;; Increment Y three positions forward to write the last value
;    lda $14
;    jsl draw_two
;    tya
;    sec
;    sbc #$000C
;    tay     ;; Skip back 6 characters to draw the top three things
    lda $16
    jsl draw_time
    plb
    plx
    rtl

;; Draw time as xx:yy:zz
draw_time:
    phx
    phb
    rep 6 : dey ;; Decrement Y by 3 characters so the time count fits
    pea $7f7f : plb : plb
    sta $004204
    sep #$20
    lda #$ff
    sta $1a
    lda #$3c
    sta $004206
    pha : pla :  pha : pla : rep #$20
    lda $004216 ;; Seconds
    sta $12
    lda $004214 ;; First two groups (hours & minutes)
    sta $004204
    sep #$20
    lda #$3c
    sta $004206
    pha : pla :  pha : pla : rep #$20
    lda $004216
    sta $14
    lda $004214 ;; First group (hours)
    jsl draw_two
    iny : iny ;; Skip past separator
    lda $14 ;; Second group (minutes)
    jsl draw_two

    iny : iny
    lda $12 ;; Last group (seconds)
    jsl draw_two

    plb
    plx
    rtl

;; Draw 5-digit value to credits tilemap
;; A = number to draw, Y = row address
draw_value:
    phx
    phb
    pea $7f7f : plb : plb
    sta $004204
    lda #$0000
    sta $1a     ;; Leading zeroes flag
    sep #$20
    lda #$64
    sta $004206
    pha : pla :  pha : pla : rep #$20
    lda $004216 ;; Last two digits
    sta $12
    lda $004214 ;; Top three digits
    jsl draw_three
    lda $12
    jsl draw_two
.end:
    plb
    plx
    rtl

draw_three:
    sta $004204
    sep #$20
    lda #$64
    sta $004206
    pha : pla :  pha : pla : rep #$20
    lda $004214 ;; Hundreds
    asl
    tax
    cmp $1a
    beq +
    lda.l numbers_top, x
    sta !credits_tilemap_offset, y
    lda.l numbers_bot, x
    sta !credits_tilemap_offset+!row, y
    dec $1a
+
    iny : iny ;; Next number
    lda $004216

draw_two:
    sta $004204
    sep #$20
    lda #$0a
    sta $004206
    pha : pla :  pha : pla : rep #$20
    lda $004214
    asl
    tax
    cmp $1a
    beq +
    lda.l numbers_top, x
    sta !credits_tilemap_offset, y
    lda.l numbers_bot, x
    sta !credits_tilemap_offset+!row, y
    dec $1a
+
    lda $004216
    asl
    tax
    cmp $1a
    beq +
    lda.l numbers_top, x
    sta !credits_tilemap_offset+2, y
    lda.l numbers_bot, x
    sta !credits_tilemap_offset+!row+2, y
    dec $1a
+
    rep 4 : iny
    rtl

;; Loop through stat table and update RAM with numbers representing those stats
write_stats:
    phy
    phb
    php
    pea $7f7f : plb : plb
    ;pea $dfdf : plb : plb
    rep #$30
    ldy #$0000

.loop:
    ;; Get pointer to table
    tya
    asl : asl : asl
    tax

    ;; Load statistic address
    lda.l stats, x
    sta $18
    lda.l stats+2, x
    sta $1A

    ;; Load stat type
    lda.l stats+6, x
    beq .end
    cmp #$0001
    beq .number
    cmp #$0002
    beq .time
    jmp .continue

.number:
    ;; Load row address
    lda.l stats+4, x
    tyx
    tay
    lda [$18]
    jsl draw_value
    txy
    jmp .continue

.time:
    ;; Load row address
    lda.l stats+4, x
    tyx
    tay
    jsl draw_full_time
    txy
    jmp .continue

.continue:
    iny
    jmp .loop

.end:
    plp
    plb
    ply
    rtl

;; 32-bit by 16-bit division routine total found somewhere
;; ($14$16)/$12 : result in $16, remainder in $14
div32:
    phy
    phx
    php
    rep #$30
    sep #$10
    sec
    lda $14
    sbc $12
    bcs .uoflo
    ldx #$11
    rep #$10
.ushftl:
    rol $16
    dex
    beq .umend
    rol $14
    lda #$0000
    rol
    sta $18
    sec
    lda $14
    sbc $12
    tay
    lda $18
    sbc #$0000
    bcc .ushftl
    sty $14
    bra .ushftl
.uoflo:
    lda #$ffff
    sta $16
    sta $14
.umend:
    plp
    plx
    ply
    rtl

numbers_top:
    dw $2060, $2061, $2062, $2063, $2064, $2065, $2066, $2067, $2068, $2069, $206a, $206b, $206c, $206d, $206e, $206f
numbers_bot:
    dw $2070, $2071, $2072, $2073, $2074, $2075, $2076, $2077, $2078, $2079, $207a, $207b, $207c, $207d, $207e, $207f



warnpc !bank_df_free_space_end

;; New credits script in free space of bank $DF
org $dfd91b
script:
    dw !set, $0002
-
    dw !draw, !blank
    dw !delay, -

    ;; Show a compact and sped up version of the original credits so we get time to add more
    ;; change scroll speed to 1 pixel per frame

    dw !speed, $0001

    dw !draw, !row*0      ;; SUPER METROID STAFF
    dw !draw, !blank
    dw !draw, !row*4      ;; PRODUCER
    dw !draw, !blank
    dw !draw, !row*7      ;; MAKOTO KANOH
    dw !draw, !row*8
    dw !draw, !blank
    dw !draw, !row*9      ;; DIRECTOR
    dw !draw, !blank
    dw !draw, !row*10     ;; YOSHI SAKAMOTO
    dw !draw, !row*11
    dw !draw, !blank
    dw !draw, !row*12     ;; BACK GROUND DESIGNERS
    dw !draw, !blank
    dw !draw, !row*13     ;; HIROFUMI MATSUOKA
    dw !draw, !row*14
    dw !draw, !blank
    dw !draw, !row*15     ;; MASAHIKO MASHIMO
    dw !draw, !row*16
    dw !draw, !blank
    dw !draw, !row*17     ;; HIROYUKI KIMURA
    dw !draw, !row*18
    dw !draw, !blank
    dw !draw, !row*19     ;; OBJECT DESIGNERS
    dw !draw, !blank
    dw !draw, !row*20     ;; TOHRU OHSAWA
    dw !draw, !row*21
    dw !draw, !blank
    dw !draw, !row*22     ;; TOMOYOSHI YAMANE
    dw !draw, !row*23
    dw !draw, !blank
    dw !draw, !row*24     ;; SAMUS ORIGINAL DESIGNERS
    dw !draw, !blank
    dw !draw, !row*25     ;; HIROJI KIYOTAKE
    dw !draw, !row*26
    dw !draw, !blank
    dw !draw, !row*27     ;; SAMUS DESIGNER
    dw !draw, !blank
    dw !draw, !row*28     ;; TOMOMI YAMANE
    dw !draw, !row*29
    dw !draw, !blank
    dw !draw, !row*83     ;; SOUND PROGRAM
    dw !draw, !row*107    ;; AND SOUND EFFECTS
    dw !draw, !blank
    dw !draw, !row*84     ;; KENJI YAMAMOTO
    dw !draw, !row*85
    dw !draw, !blank
    dw !draw, !row*86     ;; MUSIC COMPOSERS
    dw !draw, !blank
    dw !draw, !row*84     ;; KENJI YAMAMOTO
    dw !draw, !row*85
    dw !draw, !blank
    dw !draw, !row*87     ;; MINAKO HAMANO
    dw !draw, !row*88
    dw !draw, !blank
    dw !draw, !row*30     ;; PROGRAM DIRECTOR
    dw !draw, !blank
    dw !draw, !row*31     ;; KENJI IMAI
    dw !draw, !row*64
    dw !draw, !blank
    dw !draw, !row*65     ;; SYSTEM COORDINATOR
    dw !draw, !blank
    dw !draw, !row*66     ;; KENJI NAKAJIMA
    dw !draw, !row*67
    dw !draw, !blank
    dw !draw, !row*68     ;; SYSTEM PROGRAMMER
    dw !draw, !blank
    dw !draw, !row*69     ;; YOSHIKAZU MORI
    dw !draw, !row*70
    dw !draw, !blank
    dw !draw, !row*71     ;; SAMUS PROGRAMMER
    dw !draw, !blank
    dw !draw, !row*72     ;; ISAMU KUBOTA
    dw !draw, !row*73
    dw !draw, !blank
    dw !draw, !row*74     ;; EVENT PROGRAMMER
    dw !draw, !blank
    dw !draw, !row*75     ;; MUTSURU MATSUMOTO
    dw !draw, !row*76
    dw !draw, !blank
    dw !draw, !row*77     ;; ENEMY PROGRAMMER
    dw !draw, !blank
    dw !draw, !row*78     ;; YASUHIKO FUJI
    dw !draw, !row*79
    dw !draw, !blank
    dw !draw, !row*80     ;; MAP PROGRAMMER
    dw !draw, !blank
    dw !draw, !row*81     ;; MOTOMU CHIKARAISHI
    dw !draw, !row*82
    dw !draw, !blank
    dw !draw, !row*101    ;; ASSISTANT PROGRAMMER
    dw !draw, !blank
    dw !draw, !row*102    ;; KOUICHI ABE
    dw !draw, !row*103
    dw !draw, !blank
    dw !draw, !row*104    ;; COORDINATORS
    dw !draw, !blank
    dw !draw, !row*105    ;; KATSUYA YAMANO
    dw !draw, !row*106
    dw !draw, !blank
    dw !draw, !row*63     ;; TSUTOMU KANESHIGE
    dw !draw, !row*96
    dw !draw, !blank
    dw !draw, !row*89    ;; PRINTED ART WORK
    dw !draw, !blank
    dw !draw, !row*90    ;; MASAFUMI SAKASHITA
    dw !draw, !row*91
    dw !draw, !blank
    dw !draw, !row*92    ;; YASUO INOUE
    dw !draw, !row*93
    dw !draw, !blank
    dw !draw, !row*94    ;; MARY COCOMA
    dw !draw, !row*95
    dw !draw, !blank
    dw !draw, !row*99    ;; YUSUKE NAKANO
    dw !draw, !row*100
    dw !draw, !blank
    dw !draw, !row*108   ;; SHINYA SANO
    dw !draw, !row*109
    dw !draw, !blank
    dw !draw, !row*110   ;; NORIYUKI SATO
    dw !draw, !row*111
    dw !draw, !blank
    dw !draw, !row*32    ;; SPECIAL THANKS TO
    dw !draw, !blank
    dw !draw, !row*33    ;; DAN OWSEN
    dw !draw, !row*34
    dw !draw, !blank
    dw !draw, !row*35    ;; GEORGE SINFIELD
    dw !draw, !row*36
    dw !draw, !blank
    dw !draw, !row*39    ;; MASARU OKADA
    dw !draw, !row*40
    dw !draw, !blank
    dw !draw, !row*43    ;; TAKAHIRO HARADA
    dw !draw, !row*44
    dw !draw, !blank
    dw !draw, !row*47    ;; KOHTA FUKUI
    dw !draw, !row*48
    dw !draw, !blank
    dw !draw, !row*49    ;; KEISUKE TERASAKI
    dw !draw, !row*50
    dw !draw, !blank
    dw !draw, !row*51    ;; MASARU YAMANAKA
    dw !draw, !row*52
    dw !draw, !blank
    dw !draw, !row*53    ;; HITOSHI YAMAGAMI
    dw !draw, !row*54
    dw !draw, !blank
    dw !draw, !row*57    ;; NOBUHIRO OZAKI
    dw !draw, !row*58
    dw !draw, !blank
    dw !draw, !row*59    ;; KENICHI NAKAMURA
    dw !draw, !row*60
    dw !draw, !blank
    dw !draw, !row*61    ;; TAKEHIKO HOSOKAWA
    dw !draw, !row*62
    dw !draw, !blank
    dw !draw, !row*97    ;; SATOSHI MATSUMURA
    dw !draw, !row*98
    dw !draw, !blank
    dw !draw, !row*122   ;; TAKESHI NAGAREDA
    dw !draw, !row*123
    dw !draw, !blank
    dw !draw, !row*124   ;; MASAHIRO KAWANO
    dw !draw, !row*125
    dw !draw, !blank
    dw !draw, !row*45    ;; HIRO YAMADA
    dw !draw, !row*46
    dw !draw, !blank
    dw !draw, !row*112   ;; AND ALL OF R&D1 STAFFS
    dw !draw, !row*113
    dw !draw, !blank
    dw !draw, !row*114   ;; GENERAL MANAGER
    dw !draw, !blank
    dw !draw, !row*5     ;; GUMPEI YOKOI
    dw !draw, !row*6
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    
    ;; change scroll speed to 2 pixels per frame
    dw !speed, $0002

    ;; Custom randomizer credits text
    dw !draw, !row*128  ; MAP RANDO CONTRIBUTORS
    dw !draw, !blank
    dw !draw, !row*129  ; MAIN DEVELOPER
    dw !draw, !blank
    dw !draw, !row*130
    dw !draw, !row*131
    dw !draw, !blank
    dw !draw, !row*132  ; ADDITIONAL DEVELOPERS
    dw !draw, !blank
    dw !draw, !row*133
    dw !draw, !row*134
    dw !draw, !blank
    dw !draw, !row*135
    dw !draw, !row*136
    dw !draw, !blank
    dw !draw, !row*137  ; LOGIC DATA
    dw !draw, !blank
    dw !draw, !row*138
    dw !draw, !row*139
    dw !draw, !blank
    dw !draw, !row*140
    dw !draw, !row*141
    dw !draw, !blank
    dw !draw, !row*142
    dw !draw, !row*143
    dw !draw, !blank
    dw !draw, !row*144  ; SPOILER MAP
    dw !draw, !blank
    dw !draw, !row*145
    dw !draw, !row*146
    dw !draw, !blank
    dw !draw, !row*156  ; SPECIAL THANKS TO
    dw !draw, !blank
    dw !draw, !row*157
    dw !draw, !row*158
    dw !draw, !blank
    dw !draw, !row*159
    dw !draw, !row*160
    dw !draw, !blank
    dw !draw, !row*230  ; AND
    dw !draw, !blank
    dw !draw, !row*161
    dw !draw, !row*162
    dw !draw, !blank
    dw !draw, !row*231
    dw !draw, !row*232
    dw !draw, !blank
    dw !draw, !row*147  ; SUPER METROID DISASSEMBLY
    dw !draw, !blank
    dw !draw, !row*148
    dw !draw, !row*149
    dw !draw, !blank
    dw !draw, !row*150  ; SUPER METROID MOD MANUAL
    dw !draw, !blank
    dw !draw, !row*151
    dw !draw, !row*152
    dw !draw, !blank
    dw !draw, !row*233  ; SPRITESOMETHING
    dw !draw, !blank
    dw !draw, !row*237
    dw !draw, !row*238
    dw !draw, !blank
    dw !draw, !row*234  ; (Sprite name)
    dw !draw, !blank
    dw !draw, !row*235  ; (Sprite author)
    dw !draw, !row*236
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank

    ;; Set scroll speed to 3 frames per pixel
    dw !speed, $0003

    dw !draw, !row*153  ; PLAY THIS RANDOMIZER AT
    dw !draw, !blank
    dw !draw, !row*154
    dw !draw, !row*155
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank

    dw !draw, !row*223  ; RANDOMIZER SETTINGS
    dw !draw, !blank
    dw !draw, !row*224  ; SKILL ASSUMPTIONS
    dw !draw, !row*225
    dw !draw, !blank
    dw !draw, !row*226  ; ITEM PROGRESSION
    dw !draw, !row*227
    dw !draw, !blank
    dw !draw, !row*228  ; QUALITY OF LIFE
    dw !draw, !row*229
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank

    dw !draw, !row*208  ; GAMEPLAY STATISTICS
    dw !draw, !blank
    dw !draw, !row*209  ; SAVES
    dw !draw, !row*210
    dw !draw, !blank
    dw !draw, !row*211  ; DEATHS
    dw !draw, !row*212
    dw !draw, !blank
    dw !draw, !row*213  ; RELOADS
    dw !draw, !row*214
    dw !draw, !blank
    dw !draw, !row*215  ; LOADBACKS
    dw !draw, !row*216
    dw !draw, !blank
    dw !draw, !row*217  ; RESETS
    dw !draw, !row*218
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank

    ;; Set scroll speed to 5 frames per pixel
    dw !speed, $0005

    dw !draw, !row*163  ; ITEM LOCATION AND COLLECT TIME
    dw !draw, !blank
    dw !draw, !row*164
    dw !draw, !row*165
    dw !draw, !blank
    dw !draw, !row*166
    dw !draw, !row*167
    dw !draw, !blank
    dw !draw, !row*168
    dw !draw, !row*169
    dw !draw, !blank
    dw !draw, !row*170
    dw !draw, !row*171
    dw !draw, !blank
    dw !draw, !row*172
    dw !draw, !row*173
    dw !draw, !blank
    dw !draw, !row*174
    dw !draw, !row*175
    dw !draw, !blank
    dw !draw, !row*176
    dw !draw, !row*177
    dw !draw, !blank
    dw !draw, !row*178
    dw !draw, !row*179
    dw !draw, !blank
    dw !draw, !row*180
    dw !draw, !row*181
    dw !draw, !blank
    dw !draw, !row*182
    dw !draw, !row*183
    dw !draw, !blank
    dw !draw, !row*184
    dw !draw, !row*185
    dw !draw, !blank
    dw !draw, !row*186
    dw !draw, !row*187
    dw !draw, !blank
    dw !draw, !row*188
    dw !draw, !row*189
    dw !draw, !blank
    dw !draw, !row*190
    dw !draw, !row*191
    dw !draw, !blank
    dw !draw, !row*192
    dw !draw, !row*193
    dw !draw, !blank
    dw !draw, !row*194
    dw !draw, !row*195
    dw !draw, !blank
    dw !draw, !row*196
    dw !draw, !row*197
    dw !draw, !blank
    dw !draw, !row*198
    dw !draw, !row*199
    dw !draw, !blank
    dw !draw, !row*200
    dw !draw, !row*201
    dw !draw, !blank
    dw !draw, !row*202
    dw !draw, !row*203
    dw !draw, !blank
    dw !draw, !row*204
    dw !draw, !row*205
    dw !draw, !blank
    dw !draw, !row*206
    dw !draw, !row*207

    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !row*219   ; FINAL TIME
    dw !draw, !row*220
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !blank
    dw !draw, !row*221   ; THANKS FOR PLAYING
    dw !draw, !row*222


    dw !set, $0018
-
    dw !draw, !blank
    dw !delay, -

    ;; Set scroll speed to 6 frames per pixel
    dw !speed, $0006

    ;; Scroll all text off and end credits
    dw !set, $0008
-
    dw !draw, !blank
    dw !delay, -
    dw !end

warnpc !stats_table_address

org !stats_table_address
stats:
    ;; STAT DATA ADDRESS, STAT DATA BANK, TILEMAP ADDRESS, TYPE (1 = Number, 2 = Time, 3 = Skip)
    ;; Item collection times (stat data address & type will be overridden in patch.rs, based on the progression order of items)
    dw !stat_item_collection_times, $007E,  !row*164, 3
    dw !stat_item_collection_times, $007E,  !row*166, 3
    dw !stat_item_collection_times, $007E,  !row*168, 3
    dw !stat_item_collection_times, $007E,  !row*170, 3
    dw !stat_item_collection_times, $007E,  !row*172, 3
    dw !stat_item_collection_times, $007E,  !row*174, 3
    dw !stat_item_collection_times, $007E,  !row*176, 3
    dw !stat_item_collection_times, $007E,  !row*178, 3
    dw !stat_item_collection_times, $007E,  !row*180, 3
    dw !stat_item_collection_times, $007E,  !row*182, 3
    dw !stat_item_collection_times, $007E,  !row*184, 3
    dw !stat_item_collection_times, $007E,  !row*186, 3
    dw !stat_item_collection_times, $007E,  !row*188, 3
    dw !stat_item_collection_times, $007E,  !row*190, 3
    dw !stat_item_collection_times, $007E,  !row*192, 3
    dw !stat_item_collection_times, $007E,  !row*194, 3
    dw !stat_item_collection_times, $007E,  !row*196, 3
    dw !stat_item_collection_times, $007E,  !row*198, 3
    dw !stat_item_collection_times, $007E,  !row*200, 3
    dw !stat_item_collection_times, $007E,  !row*202, 3
    dw !stat_item_collection_times, $007E,  !row*204, 3
    dw !stat_item_collection_times, $007E,  !row*206, 3
    dw !stat_saves,     $0070, !row*209,  1    ;; Saves
    dw !stat_deaths,    $0070, !row*211,  1    ;; Deaths
    dw !stat_reloads,   $0070, !row*213,  1    ;; Reloads
    dw !stat_loadbacks, $0070, !row*215,  1    ;; Loadbacks
    dw !stat_resets,    $0070, !row*217,  1    ;; Resets
    dw !stat_final_time,     $0070, !row*219,  2    ;; Final time
    dw 0,              0,  0, 0    ;; (End of table)

warnpc $e00000

;; Relocated credits tilemap to free space in bank CE
org !bank_ce_free_space_start
credits:
    !pink
    dw "     MAP RANDO CONTRIBUTORS     " ;; 128
    !yellow
    dw "         MAIN DEVELOPER         " ;; 129
    !big
    dw "             MADDO              " ;; 130
    dw "             maddo              " ;; 131
    !yellow
    dw "     ADDITIONAL DEVELOPERS      " ;; 132
    !big
    dw "             KYLEB              " ;; 133
    dw "             kyleb              " ;; 134
    dw "            OSSE101             " ;; 135
    dw "            osse!}!             " ;; 136
    !orange
    dw "    LOGIC DATA MAIN AUTHORS     " ;; 137
    !big
    dw "    RUSHLIGHT        OSSE101    " ;; 138
    dw "    rushlight        osse!}!    " ;; 139
    dw "    MATRETHEWEY      KYLEB      " ;; 140
    dw "    matrethewey      kyleb      " ;; 141
    dw "    DIPROGAN                    " ;; 142
    dw "    diprogan                    " ;; 143
    !cyan
    dw "          SPOILER MAP           " ;; 144
    !big
    dw "            SELICRE             " ;; 145
    dw "            selicre             " ;; 146
    !purple
    dw "   SUPER METROID DISASSEMBLY    " ;; 147
    !big
    dw "      PJBOY      KEJARDON       " ;; 148
    dw "      pjboy      kejardon       " ;; 149
    !purple
    dw "    SUPER METROID MOD MANUAL    " ;; 150
    !big
    dw "            BEGRIMED            " ;; 151
    dw "            begrimed            " ;; 152
    !green
    dw "    PLAY THIS RANDOMIZER AT     " ;; 153
    !big
    dw "          MAPRANDO COM          " ;; 154
    dw "          maprando.com          " ;; 155
    !cyan
    dw "       SPECIAL THANKS TO        " ;; 156
    !big
    dw "   BUGGMANN         SOMERANDO   " ;; 157
    dw "   buggmann         somerando   " ;; 158
    dw "   BOBBOB       INSOMNIASPEED   " ;; 159
    dw "   bobbob       insomniaspeed   " ;; 160
    dw "   ALL SUPER METROID HACKERS    " ;; 161
    dw "   all super metroid hackers    " ;; 162
    !blue
    dw " ITEM LOCATION AND COLLECT TIME " ;; 163
    !big
    dw "                                " ;; 164
    dw "                                " ;; 165
    dw "                                " ;; 166
    dw "                                " ;; 167
    dw "                                " ;; 168
    dw "                                " ;; 169
    dw "                                " ;; 170
    dw "                                " ;; 171
    dw "                                " ;; 172
    dw "                                " ;; 173
    dw "                                " ;; 174
    dw "                                " ;; 175
    dw "                                " ;; 176
    dw "                                " ;; 177
    dw "                                " ;; 178
    dw "                                " ;; 179
    dw "                                " ;; 180
    dw "                                " ;; 181
    dw "                                " ;; 182
    dw "                                " ;; 183
    dw "                                " ;; 184
    dw "                                " ;; 185
    dw "                                " ;; 186
    dw "                                " ;; 187
    dw "                                " ;; 188
    dw "                                " ;; 189
    dw "                                " ;; 190
    dw "                                " ;; 191
    dw "                                " ;; 192
    dw "                                " ;; 193
    dw "                                " ;; 194
    dw "                                " ;; 195
    dw "                                " ;; 196
    dw "                                " ;; 197
    dw "                                " ;; 198
    dw "                                " ;; 199
    dw "                                " ;; 200
    dw "                                " ;; 201
    dw "                                " ;; 202
    dw "                                " ;; 203
    dw "                                " ;; 204
    dw "                                " ;; 205
    dw "                                " ;; 206
    dw "                                " ;; 207
    !blue
    dw "      GAMEPLAY STATISTICS       " ;; 208
    !big
    dw " SAVES                        0 " ;; 209
    dw " saves                        } " ;; 210
    dw " DEATHS                       0 " ;; 211
    dw " deaths                       } " ;; 212
    dw " RELOADS                      0 " ;; 213
    dw " reloads                      } " ;; 214
    dw " LOADBACKS                    0 " ;; 215
    dw " loadbacks                    } " ;; 216
    dw " RESETS                       0 " ;; 217
    dw " resets                       } " ;; 218
    dw " FINAL TIME            00.00.00 " ;; 219
    dw " final time            }}.}}.}} " ;; 220
    dw "       THANKS FOR PLAYING       " ;; 221
    dw "       thanks for playing       " ;; 222
    !blue
    dw "      RANDOMIZER SETTINGS       " ;; 223
    !big
    dw " SKILL ASSUMPTIONS              " ;; 224
    dw " skill assumptions              " ;; 225
    dw " ITEM PROGRESSION               " ;; 226
    dw " item progression               " ;; 227
    dw " QUALITY OF LIFE                " ;; 228
    dw " quality of life                " ;; 229
    !cyan
    dw "              AND               " ;; 230
    !big
    dw "   SM RANDOMIZER COMMUNITIES    " ;; 231
    dw "   sm randomizer communities    " ;; 232
    !purple
    dw "        SPRITESOMETHING         " ;; 233
    !yellow
    dw "                                " ;; 234 - sprite name (to be filled in by randomizer if custom sprite used)
    !big
    dw "                                " ;; 235 - sprite author (to be filled in by randomizer if custom sprite used)
    dw "                                " ;; 236 - sprite author (to be filled in by randomizer if custom sprite used)
    !big
    dw "     ARTHEAU    MATRETHEWEY     " ;; 237
    dw "     artheau    matrethewey     " ;; 238

    dw $0000
warnpc !bank_ce_free_space_end
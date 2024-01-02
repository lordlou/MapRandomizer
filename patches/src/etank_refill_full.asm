lorom

org $848972
	jsr $F760

org $84F760
	sta $09C2 ;} Original instruction
	lda $09D4 ;\
	sta $09D6 ;} Refill reserves
	rts

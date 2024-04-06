lorom

!bank_84_free_space_start = $84F760
!bank_84_free_space_end = $84F770

org $848972
	jsr refill_hook

org !bank_84_free_space_start
refill_hook:
	sta $09C2 ;} Original instruction
	lda $09D4 ;\
	sta $09D6 ;} Refill reserves
	rts

warnpc !bank_84_free_space_end
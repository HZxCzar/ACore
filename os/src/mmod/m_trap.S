.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text.mtraps
    .globl __alltraps_m
    .globl __restore_m
    .align 2
__alltraps_m:
    csrrw sp, mscratch, sp
    addi sp, sp, -34*8
    sd x1, 1*8(sp)
    sd x3, 3*8(sp)
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    csrr t0, mstatus
    csrr t1, mepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    csrr t2, mscratch
    sd t2, 2*8(sp)
    call trap_handler_m

__restore_m:
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw mstatus, t0
    csrw mepc, t1
    csrw mscratch, t2
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    addi sp, sp, 34*8
    csrrw sp, mscratch, sp
    mret

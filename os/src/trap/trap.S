.altmacro
.macro SAVE_GP_S n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP_S n
    ld x\n, \n*8(sp)
.endm
    .section .text.trampoline
    .globl __alltraps_s
    .globl __restore_s
    .align 2
__alltraps_s:
    csrrw sp, sscratch, sp
    sd x1, 1*8(sp)
    sd x3, 3*8(sp)
    .set n, 5
    .rept 27
        SAVE_GP_S %n
        .set n, n+1
    .endr
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    csrr t2, sscratch
    sd t2, 2*8(sp)
    ld t0, 34*8(sp)
    ld t1, 36*8(sp)
    ld sp, 35*8(sp)
    csrw satp, t0
    sfence.vma
    jr t1

__restore_s:
    csrw satp, a1
    sfence.vma
    csrw sscratch, a0
    mv sp, a0
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP_S %n
        .set n, n+1
    .endr
    ld sp, 2*8(sp)
    sret

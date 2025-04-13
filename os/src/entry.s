    .section .text.entry
    .globl _start
_start:
    la sp, m_stack_top
    call m_mode_init

    .section .bss.mstack
    .globl m_stack_lower_bound
m_stack_lower_bound:
    .space 4096 * 16
    .globl m_stack_top
m_stack_top:

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:
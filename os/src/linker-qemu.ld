OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80000000;
KENL_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;
    smtext = .;
    .mtext : {
        *(.text.entry)
        *(.text.minit)
        *(.text.mtraps)
    }
    . = ALIGN(4K);
    emtext = .;
    smstack = .;
    .mbss : {
        *(.bss.mstack)
    }
    . = ALIGN(4K);
    emstack = .;

    . = KENL_ADDRESS;
    skernel = .;

    stext = .;
    .text : {
        strampoline = .;
        *(.text.trampoline);
        . = ALIGN(4K);
        *(.text .text.*)
    }

    . = ALIGN(4K);
    etext = .;
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    edata = .;
    sbss_with_stack = .;
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}
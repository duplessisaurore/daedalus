ENTRY(_start)

MEMORY
{
    RAM (rwx) : ORIGIN = 0x00000000, LENGTH = 2048M
}

DAEDALUS_BASE = 0x08000000;
HEAP_SIZE  = 0x1000000;
STACK_SIZE = 0x0100000;
GUARD_SIZE = 0x0001000;

SECTIONS
{
    .text DAEDALUS_BASE : ALIGN(4)
    {
        __daedalus_start = .;

        KEEP(*(.text._start))
        *(.text*)
    } > RAM

    .rodata : ALIGN(8)
    {
        *(.rodata*)
    } > RAM

    .data : ALIGN(8)
    {
        *(.data*)
    } > RAM

    .bss : ALIGN(8)
    {
        __bss_start = .;
        *(.bss*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    . = ALIGN(16);
    __heap_start = .;
    . += HEAP_SIZE;
    __heap_end = .;

    . = ALIGN(4096);
    __stack_guard_start = .;
    . += GUARD_SIZE;
    __stack_guard_end = .;

    __stack_bottom = .;
    . += STACK_SIZE;
    __stack_top = .;

    . = ALIGN(4096);
    __daedalus_end = .;

    __dram_end = ORIGIN(RAM) + LENGTH(RAM);

    ASSERT(__daedalus_end <= __dram_end,
           "daedalus does not fit inside the declared RAM region")
}
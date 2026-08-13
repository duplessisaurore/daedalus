// startup.S
//
// `Daedalus` expects to be entered at EL1, in non-secure mode.
//

.global _start
.section .text._start, "ax"
_start:
    // Whatever the previous stage left in x0, for future usage if needed?
    mov x19, x0

    // set up the stack pointer to point to the expected one in `Daedalus`
    adrp x0, __stack_top
    add  x0, x0, :lo12:__stack_top
    mov  sp, x0

    // give rust entry the previous stages x0
    mov x0, x19

    bl rust_entry

    // infinite loop if rust_entry returns
    // because it shouldnt..., and this is easier to debug
1:
    b 1b
// This is the exception vector table for the `GICv2` IrqArch

// https://jcomes.org/aarch64-os-interrupt-handling-i

// Save all registers to the stack
.macro SAVE_CONTEXT
    sub sp, sp, #272
    stp x0,  x1,  [sp, #16 * 0]
    stp x2,  x3,  [sp, #16 * 1]
    stp x4,  x5,  [sp, #16 * 2]
    stp x6,  x7,  [sp, #16 * 3]
    stp x8,  x9,  [sp, #16 * 4]
    stp x10, x11, [sp, #16 * 5]
    stp x12, x13, [sp, #16 * 6]
    stp x14, x15, [sp, #16 * 7]
    stp x16, x17, [sp, #16 * 8]
    stp x18, x19, [sp, #16 * 9]
    stp x20, x21, [sp, #16 * 10]
    stp x22, x23, [sp, #16 * 11]
    stp x24, x25, [sp, #16 * 12]
    stp x26, x27, [sp, #16 * 13]
    stp x28, x29, [sp, #16 * 14]
    mrs x0, elr_el2
    mrs x1, spsr_el2
    stp x30, x0,  [sp, #16 * 15]
    str x1,       [sp, #16 * 16]
.endm

// Put back all registers from the stack
.macro RESTORE_CONTEXT
    ldr x1,       [sp, #16 * 16]
    ldp x30, x0,  [sp, #16 * 15]
    msr elr_el2, x0
    msr spsr_el2, x1
    ldp x28, x29, [sp, #16 * 14]
    ldp x26, x27, [sp, #16 * 13]
    ldp x24, x25, [sp, #16 * 12]
    ldp x22, x23, [sp, #16 * 11]
    ldp x20, x21, [sp, #16 * 10]
    ldp x18, x19, [sp, #16 * 9]
    ldp x16, x17, [sp, #16 * 8]
    ldp x14, x15, [sp, #16 * 7]
    ldp x12, x13, [sp, #16 * 6]
    ldp x10, x11, [sp, #16 * 5]
    ldp x8,  x9,  [sp, #16 * 4]
    ldp x6,  x7,  [sp, #16 * 3]
    ldp x4,  x5,  [sp, #16 * 2]
    ldp x2,  x3,  [sp, #16 * 1]
    ldp x0,  x1,  [sp, #16 * 0]
    add sp, sp, #272
.endm

.section .text.vectors, "ax"

.macro vector label
    .p2align 7
    b \label
.endm

// vector table must be 2048-byte aligned
.p2align 11
.global daedalus_vectors
daedalus_vectors:
    // SP_EL0, we never get here so things should be unexpected
    vector   daedalus_unexpected_exception 
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception

    // SP_ELx, this is where we get irqs from
    vector   daedalus_unexpected_exception
    vector   irq_entry
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception

    // Lower EL aarch64, nothing should run below daedalus so we dont expect anything here
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception

    // Lower EL aarch32
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception
    vector   daedalus_unexpected_exception

// We need to save and restore the prior context of the exception
// so when returning using `eret` whatever we clobbered in daedalus_irq_handler
// is restored so stuff works ^_^
irq_entry:
    SAVE_CONTEXT
    bl  daedalus_irq_handler
    RESTORE_CONTEXT
    eret
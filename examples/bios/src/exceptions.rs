use crate::println;
use crate::thread::{get_current_thread, reschedule_threads, set_current_thread, ThreadControlBlock};
use core::arch::asm;
use psx::hw::cop0;
use psx::hw::cop0::{Excode, IntSrc};
use psx::hw::irq;
use psx::hw::Register;
use psx::irq::IRQ;
use psx::sys::kernel::*;

#[naked]
pub unsafe extern "C" fn exception_vec() {
    asm! {
        ".set noreorder
         .set noat
         la $k0, exception_handler
         jr $k0
         nop
         .set at
         .set reorder",
        options(noreturn)
    }
}

#[naked]
#[no_mangle]
pub unsafe extern "C" fn exception_handler() {
    asm! {
        ".set noreorder
         .set noat
         la $k0, CURRENT_THREAD
         lw $k0, ($k0)
         nop

         sw $at, ($k0)

         sw $v0, 4($k0)
         sw $v1, 8($k0)

         sw $a0, 12($k0)
         sw $a1, 16($k0)
         sw $a2, 20($k0)
         sw $a3, 24($k0)

         sw $t0, 28($k0)
         sw $t1, 32($k0)
         sw $t2, 36($k0)
         sw $t3, 40($k0)
         sw $t4, 44($k0)
         sw $t5, 48($k0)
         sw $t6, 52($k0)
         sw $t7, 56($k0)

         sw $t8, 92($k0)
         sw $t9, 96($k0)

         sw $ra, 120($k0)

         mflo $t0
         mfhi $t1
         mfc0 $t2, $12
         mfc0 $t3, $13
         mfc0 $t4, $14
         sw $t0, 124($k0)
         sw $t1, 128($k0)
         sw $t2, 132($k0)
         sw $t3, 136($k0)
         sw $t4, 140($k0)

         # call_handlers is in ROM so we need jalr
         la $k1, call_handlers
         jalr $k1
         nop

         # Check if we switched threads
         beqz $v0, 2f
         # Get the new TCB pointer
         la $k1, CURRENT_THREAD
         lw $k1, ($k1)

         sw $s0, 60($k0)
         sw $s1, 64($k0)
         sw $s2, 68($k0)
         sw $s3, 72($k0)
         sw $s4, 76($k0)
         sw $s5, 80($k0)
         sw $s6, 84($k0)
         sw $s7, 88($k0)
         sw $sp, 112($k0)
         sw $fp, 116($k0)

         lw $s0, 60($k1)
         lw $s1, 64($k1)
         lw $s2, 68($k1)
         lw $s3, 72($k1)
         lw $s4, 76($k1)
         lw $s5, 80($k1)
         lw $s6, 84($k1)
         lw $s7, 88($k1)
         lw $sp, 112($k1)
         j 3f
         lw $fp, 116($k1) # jump delay slot

         2:
         move $k1, $k0
         3:

         lw $at, ($k1)

         lw $t0, 124($k1)
         lw $t1, 128($k1)
         lw $t2, 132($k1)
         lw $t3, 136($k1)
         lw $t4, 140($k1)
         mtlo $t0
         mthi $t1
         mtc0 $t2, $12
         mtc0 $t3, $13
         move $k0, $t4

         lw $v0, 4($k1)
         lw $v1, 8($k1)

         lw $a0, 12($k1)
         lw $a1, 16($k1)
         lw $a2, 20($k1)
         lw $a3, 24($k1)

         lw $t0, 28($k1)
         lw $t1, 32($k1)
         lw $t2, 36($k1)
         lw $t3, 40($k1)
         lw $t4, 44($k1)
         lw $t5, 48($k1)
         lw $t6, 52($k1)
         lw $t7, 56($k1)

         lw $t8, 92($k1)
         lw $t9, 96($k1)

         lw $ra, 120($k1)

         jr $k0
         .long 0x42000010 #rfe
         .set at
         .set reorder",
         options(noreturn)
    }
}

#[no_mangle]
extern "C" fn call_handlers() -> u32 {
    // SAFETY: This is safe to call in the exception handler
    let tcb = unsafe { get_current_thread() };
    let excode = cop0::Cause::from_bits(*tcb.cop0_cause()).excode();
    let switched = match excode {
        Excode::Interrupt => irq_handler(),
        Excode::Syscall => syscall_handler(tcb),
        Excode::Breakpoint => {
            println!("{:#x?}", tcb);
            *tcb.cop0_epc() += 4;
            false
        },
        _ => {
            println!("No handler installed for exception code {excode:?}");
            false
        },
    };
    switched as u32
}

fn irq_handler() -> bool {
    let mut stat = irq::Status::new();
    let mask = irq::Mask::new();

    let mut changed_threads = false;
    for irq in mask.active_irqs(&stat) {
        if let Some(irq) = irq {
            match irq {
                IRQ::Vblank => {
                    changed_threads = vblank_handler();
                },
                _ => {
                    println!("No handler installed for interrupt {irq:?}");
                },
            }
        }
    }
    stat.ack_all().store();
    changed_threads
}

fn syscall_handler(tcb: &mut ThreadControlBlock) -> bool {
    *tcb.cop0_epc() += 4;
    match tcb.regs[3] as u8 {
        ENTER_CRITICAL_SECTION_NUM => {
            cop0::Status::new()
                .disable_interrupts()
                .mask_interrupt(IntSrc::Hardware)
                .store();
        },
        EXIT_CRITICAL_SECTION_NUM => {
            cop0::Status::new()
                .enable_interrupts()
                .unmask_interrupt(IntSrc::Hardware)
                .store();
        },
        CHANGE_THREAD_SUB_FN_NUM => {
            // SAFETY: This is safe to call in the exception handler
            unsafe { set_current_thread(tcb.regs[4] as *mut ThreadControlBlock) };
            return true
        },
        _ => (),
    };
    false
}

// A vblank handler repurposed to schedule threads
fn vblank_handler() -> bool {
    unsafe {
        // SAFETY: This is safe to call in the exception handler
        reschedule_threads()
    }
}

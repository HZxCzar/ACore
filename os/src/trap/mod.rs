//! Trap handling functionality
//!
//! For rCore, we have a single trap entry point, namely `__alltraps`. At
//! initialization in [`init()`], we set the `stvec` CSR to point to it.
//!
//! All traps go through `__alltraps`, which is defined in `trap.S`. The
//! assembly language code does just enough work restore the kernel space
//! context, ensuring that Rust code safely runs, and transfers control to
//! [`trap_handler()`].
//!
//! It then calls different functionality based on what exactly the exception
//! was. For example, timer interrupts trigger task preemption, and syscalls go
//! to [`syscall()`].

mod context;

// use crate::batch::run_next_app;
// use crate::syscall::syscall;
use core::arch::global_asm;
use riscv::register::{
    mcause::{self,Interrupt},mtvec::{self, TrapMode}, scause::{self, Exception,Interrupt::SupervisorTimer}, stval, stvec,mtval
};

global_asm!(include_str!("trap.S"));

/// initialize CSR `stvec` as the entry of `__alltraps`
pub fn init() {
    unsafe extern "C" {
        safe fn __alltraps_s();
        safe fn __alltraps_m();
    }
    unsafe {
        stvec::write(__alltraps_s as usize, TrapMode::Direct);
        mtvec::write(__alltraps_m as usize, TrapMode::Direct);
    }
}

#[unsafe(no_mangle)]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler_s(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read(); // get trap cause
    let stval = stval::read(); // get extra value
    match scause.cause() {
        scause::Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            // cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        scause::Trap::Exception(Exception::StoreFault) | scause::Trap::Exception(Exception::StorePageFault) => {
            println!("[kernel] PageFault in application, kernel killed it.");
            // run_next_app();
        }
        scause::Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            // run_next_app();
        }
        scause::Trap::Interrupt(SupervisorTimer)=>{
            set_next_trigger();
            panic!("|timer_interrupt|");
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx
}

#[unsafe(no_mangle)]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler_m(cx: &mut TrapContext) -> &mut TrapContext {
    let mcause = mcause::read(); // get trap cause
    let mtval = mtval::read(); // get extra value
    match mcause.cause() {
        mcause::Trap::Exception(mcause::Exception::UserEnvCall) => {
            cx.sepc += 4;
            // cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        mcause::Trap::Interrupt(Interrupt::MachineTimer) => {
            set_next_trigger();
            panic!("|m_timer_interrupt|");
            // cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, mtval = {:#x}!",
                mcause.cause(),
                mtval
            );
        }
    }
    cx
}

pub use context::TrapContext;

use crate::timer::set_next_trigger;

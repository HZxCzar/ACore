mod context;

use crate::config::{TRAMPOLINE, TRAP_CONTEXT};
use crate::mm::VirtAddr;
use crate::syscall::syscall;
use crate::task::{
    check_signals_error_of_current, current_add_signal, current_trap_cx, current_user_token,
    exit_current_and_run_next, handle_signals, suspend_current_and_run_next, SignalFlags,
};
use core::arch::{asm, global_asm};
use riscv::register::{
    mcause::{self, Interrupt},
    mtval,
    mtvec::TrapMode,
    scause::{
        self, Exception,
        Interrupt::{SupervisorSoft, SupervisorTimer},
        Trap,
    },
    stval, stvec,
};

global_asm!(include_str!("trap.S"));

/// initialize CSR `stvec` as the entry of `__alltraps`
pub fn init() {
    set_kernel_trap_entry();
    println!("init trap");
}

fn set_kernel_trap_entry() {
    // unsafe extern "C" {
    //     safe fn __alltraps_s();
    //     safe fn __alltraps_m();
    // }
    // unsafe {
    //     stvec::write(__alltraps_s as usize - __alltraps_m as usize + TRAMPOLINE, TrapMode::Direct);
    // }
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
        println!("stvec: {:?}", stvec::read());
    }
}

#[unsafe(no_mangle)]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler_s() -> ! {
    let cx = current_trap_cx();
    let scause = scause::read(); // get trap cause
    let stval = stval::read(); // get extra value
    // println!("|s_interrupt|");
    // println!(
    //     "[kernel] trap_handler_s: scause = {:?}, stval = {:#x}, sepc = {:#x}",
    //     scause.cause(),
    //     stval,
    //     cx.sepc
    // );
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
           // jump to next instruction anyway
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            // get system call return value
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]);
            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StorePageFault) => {
            // println!(
            //     "[kernel] StorePageFault in application, bad addr = {:#x}, bad instruction = {:#x}",
            //     stval, cx.sepc
            // );
            let fault_addr = VirtAddr(stval::read());
            if !crate::task::handle_cow(fault_addr) {
                println!("[kernel] StorePageFault in application and not cow");
                current_add_signal(SignalFlags::SIGSEGV);
            }
            // else{
            //     println!("[kernel] StorePageFault in application and cow");
            // }
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            /*
            println!(
                "[kernel] {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            */
            current_add_signal(SignalFlags::SIGSEGV);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            // println!("[kernel] IllegalInstruction in application, kernel killed it.");
            // println!(
            //     "[kernel] bad instruction = {:#x}, bad addr = {:#x}",
            //     cx.sepc, stval
            // );
            current_add_signal(SignalFlags::SIGILL);
        }
        Trap::Interrupt(SupervisorTimer) => {
            println!("|s_timer_interrupt|");
            set_next_trigger();
            suspend_current_and_run_next();
        }
        Trap::Interrupt(SupervisorSoft) => {
            // println!("|s_soft_interrupt|");
            unsafe {
                // 读取当前sip值
                let sip = riscv::register::sip::read().bits();
                // 将SSIP位(第1位)清除
                asm!("csrw sip, {sip}", sip = in(reg) sip & !2);
                // panic!("|s_soft_interrupt|");
            }
            // 时间片轮转
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    handle_signals();
    if let Some((errno, msg)) = check_signals_error_of_current() {
        println!("[kernel] {}", msg);
        exit_current_and_run_next(errno);
    }
    trap_return_s();
}

fn set_user_trap_entry() {
    unsafe extern "C" {
        safe fn __alltraps_s();
    }
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

#[unsafe(no_mangle)]
/// set the new addr of __restore asm function in TRAMPOLINE page,
/// set the reg a0 = trap_cx_ptr, reg a1 = phy addr of usr page table,
/// finally, jump to new addr of __restore asm function
pub fn trap_return_s() -> ! {
    set_user_trap_entry();
    // println!("|s_trap_return|");
    let trap_cx_ptr = TRAP_CONTEXT;
    let user_satp = current_user_token();
    // println!("|s_trap_return|");
    unsafe extern "C" {
        safe fn __alltraps_s();
        safe fn __restore_s();
    }
    let restore_va = __restore_s as usize - __alltraps_s as usize + TRAMPOLINE;
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",             // jump to new addr of __restore asm function
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,      // a0 = virt addr of Trap Context
            in("a1") user_satp,        // a1 = phy addr of usr page table
            options(noreturn)
        );
    }
}

#[unsafe(no_mangle)]
/// Unimplement: traps/interrupts/exceptions from kernel mode
/// Todo: Chapter 9: I/O device
pub fn trap_from_kernel() -> ! {
    println!("|kernel_trap|");
    // unsafe { core::arch::asm!("ecall"); }
    panic!("a trap from kernel!");
}

pub use context::TrapContext;

use crate::timer::set_next_trigger;

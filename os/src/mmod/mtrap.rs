use crate::config::{TRAMPOLINE, TRAP_CONTEXT};
// use crate::syscall::syscall;
use core::arch::asm;
use crate::task::{
    current_trap_cx, current_user_token
};
use riscv::register::{
    mcause::{self,Interrupt},mtvec::TrapMode,stvec,mtval,mip,mepc,mstatus::{self,MPP}
};
use crate::mmod::mtimer::set_next_trigger;
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.mtraps")]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler_m(){
    // let mip = mip::read(); // get interrupt pending
    let mcause = mcause::read(); // get trap cause
    let mtval = mtval::read(); // get extra value
    // let mstatus = mstatus::read(); // get mstatus
    // println!("|m_interrupt|");
    unsafe extern "C" {
        safe fn __restore_m();
    }
    // let restore_va = __restore_m as usize;
    match mcause.cause() {
        mcause::Trap::Interrupt(Interrupt::MachineTimer) => {
                // 从 U 模式跳转过来的情况下执行的操作
                set_next_trigger();
                unsafe {
                    asm!(
                        "csrw sip, 2",
                        // "li t0, (1 << 63) | 5", // 设置scause为时钟中断: MSB=1(表示中断), 其余位=5(S模式时钟中断)
                        // "csrw scause, t0",
                    );
                }
                // 可以在这里添加更多只针对 U 模式触发的中断处理代码
                // println!("|m_timer_interrupt_from_user|");
            // set_next_trigger();
            // unsafe {
            //     asm!(
            //         "csrw sip, 2",
            //     );
            // }
            // // println!("|m_timer_interrupt|");
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
    __restore_m();
}

// #[unsafe(no_mangle)]
// #[unsafe(link_section = ".mtext.mtraps")]
// /// set the new addr of __restore asm function in TRAMPOLINE page,
// /// set the reg a0 = trap_cx_ptr, reg a1 = phy addr of usr page table,
// /// finally, jump to new addr of __restore asm function
// pub fn trap_return_m() -> ! {
//     // let trap_cx_ptr = TRAP_CONTEXT;
//     // let user_satp = current_user_token();
//     // println!("|m_trap_return|");
//     unsafe {
//         asm!(
//             "fence.i",
//             "jr {restore_va}",             // jump to new addr of __restore asm function
//             restore_va = in(reg) restore_va,
//             // in("a0") trap_cx_ptr,      // a0 = virt addr of Trap Context
//             // in("a1") user_satp,        // a1 = phy addr of usr page table
//             options(noreturn)
//         );
//     }
// }

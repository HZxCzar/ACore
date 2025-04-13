use core::arch::asm;
use core::arch::global_asm;
use riscv::register::mscratch;
use riscv::register::mstatus::MPP;
use riscv::register::sstatus;
use riscv::register::{mcause, mepc, mie, mstatus, mtvec, pmpaddr0, pmpcfg0, satp, sie};

use crate::config::TRAP_CONTEXT;
use crate::config::TRAMPOLINE;
use crate::trap::TrapContext;
// struct MachineStack {
//     data: [u8; MACHINE_STACK_SIZE],
// }
// static MACHINE_STACK: MachineStack = MachineStack {
//     data: [0; MACHINE_STACK_SIZE],
// };
// impl MachineStack {
//     fn get_sp(&self) -> usize {
//         self.data.as_ptr() as usize + MACHINE_STACK_SIZE
//     }
//     pub fn push_context(&self, cx: TrapContext) -> &'static mut TrapContext {
//         let cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
//         unsafe {
//             *cx_ptr = cx;
//         }
//         unsafe { cx_ptr.as_mut().unwrap() }
//     }
// }

global_asm!(include_str!("m_trap.S"));
unsafe extern "C" {
    unsafe fn rust_main() -> !; // S 模式入口点
    safe fn __alltraps_m(); // 中断处理
    safe fn smstack(); // M 模式栈
    safe fn emstack(); // M 模式栈
}

mod mtrap;
mod mtimer;
/// 从 M 模式切换到 S 模式
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.minit")]
pub unsafe fn m_mode_init() -> ! {
    mstatus::set_mpp(MPP::Supervisor); // 设置 MPP=01 (S 模式)
    // 设置 S 模式入口点为 rust_main
    mepc::write(rust_main as usize);

    mscratch::write(emstack as usize); // 设置 M 模式栈指针

    // 禁用分页
    satp::write(0);

    
    // unsafe {
    //     // 设置异常委托，但排除S模式ECALL(位9)
    //     let medeleg_value = 0xfdff; // 0xffff & ~(1 << 9)
    //     asm!(
    //         "csrw medeleg, {0}",
    //         "li {1}, 0xffff", 
    //         "csrw mideleg, {1}",
    //         in(reg) medeleg_value,
    //         out(reg) _,
    //     );
    // }


    // 委托异常和中断到 S 模式
    unsafe {
        let mut t0: usize;
        asm!(
            "li {0}, 0xffff",
            "csrw medeleg, {0}",
            "csrw mideleg, {0}",
            out(reg) t0
        );
    }

    // 启用 S 模式中断
    unsafe {
        sstatus::set_sie(); // 设置 SIE (S-mode Interrupt Enable)
        sie::set_sext(); // 设置 SEIE (S-mode External Interrupt Enable)
        sie::set_stimer(); // 设置 STIE (S-mode Timer Interrupt Enable)
        sie::set_ssoft(); // 设置 SSIE (S-mode Software Interrupt Enable)
    }

    pmpaddr0::write(0x3fffffffffffff);
    pmpcfg0::write(0xf);

    // 设置时钟中断
    mtimer::set_timer();

    unsafe {
        unsafe extern "C" {
            safe fn __alltraps_m();
        }
        mtvec::write(__alltraps_m as usize, mtvec::TrapMode::Direct);
        // unsafe extern "C" {
        //     safe fn __alltraps_s();
        //     safe fn __alltraps_m();
        // }
        // unsafe extern "C" {
        //     safe fn __alltraps_m();
        // }
        // mtvec::write(TRAMPOLINE as usize, mtvec::TrapMode::Direct);
        // 设置时钟中断处理函数
        // mtvec::write(TRAMPOLINE as usize, mtvec::TrapMode::Direct);

        mstatus::set_mie();

        mie::set_mtimer();
    }
    unsafe {
        asm!("la sp, boot_stack_top");
    }
    // 执行 mret 指令切换到 S 模式
    unsafe {
        asm!("mret", options(noreturn));
    }
}

// /// 时钟中断处理程序
// ///
// /// 这个函数需要一个汇编包装器，因为直接的中断处理需要保存和恢复上下文
// #[unsafe(no_mangle)]
// pub unsafe fn timer_trap_handler() {
//     // 重置计时器
//     const MTIME_ADDR: usize = 0x0200bff8;
//     const MTIMECMP_ADDR: usize = 0x02004000;
//     const INTERVAL: u64 = 10000000; // 大约1秒
//     panic!("[kernel] timer interrupt");
//     // 读取当前时间
//     let mtime = (MTIME_ADDR as *const u64).read_volatile();

//     // 设置下一次中断时间
//     let next_time = mtime + INTERVAL;
//     (MTIMECMP_ADDR as *mut u64).write_volatile(next_time);
// }

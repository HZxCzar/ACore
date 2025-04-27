#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
mod console;
mod lang_items;
mod uart;
mod timer;
mod mm;
mod mmod;
mod config;
mod loader;
pub mod trap;
pub mod sync;
pub mod task;
pub mod syscall;

// use config::TRAMPOLINE;
// use riscv::register::mtvec;
use core::arch::global_asm;
global_asm!(include_str!("entry.s"));
global_asm!(include_str!("link_app.S"));

use uart::Uart;
// static mut UART_INSTANCE: uart::Uart = unsafe { uart::Uart::new(0x1000_0000) };
static mut UART_INSTANCE: Option<Uart> = None;

fn clear_bss() {
    unsafe extern "C" {
        safe fn sbss();
        safe fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}

fn uart_init() {
    unsafe {
        UART_INSTANCE = Some(Uart::new(0x1000_0000));
        if let Some(ref mut uart) = UART_INSTANCE {
            uart.init();
        }
        console::set_uart();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mmode_start() -> ! {
    // 从 M 模式切换到 S 模式
    unsafe { mmod::m_mode_init() }
    
    // 不应该到达这里
    panic!("|mmode_start_error|");
}

pub fn dump_memory(addr: usize, size: usize) {
    println!("内存内容 [{:#x}..{:#x}]:", addr, addr + size);
    
    unsafe {
        // 将地址解释为u32指针（RISC-V指令是32位的）
        let code_ptr = addr as *const u32;
        
        // 尝试读取并打印指令
        for i in 0..(size / 4) {
            let instruction = *code_ptr.add(i);
            println!("{:#x}: {:#010x}", addr + i * 4, instruction);
        }
    }
}

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    clear_bss();
    uart_init();
    mm::init();
    mm::remap_test();
    task::add_initproc();
    println!("after initproc!");
    trap::init();
    timer::set_next_trigger();
    loader::list_apps();
    task::run_tasks();
    // println!("准备进入M模式trap处理程序...");
    // unsafe { core::arch::asm!("ecall"); }  // 添加这一行触发M模式trap
    // while true {
        
    // }
    // println!("|rust_main|");
    // task::run_tasks();
    panic!("|program finished|");
}
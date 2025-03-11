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
pub mod trap;
pub mod sync;
// pub mod syscall;
use core::arch::global_asm;
global_asm!(include_str!("entry.s"));

use uart::Uart;
// static mut UART_INSTANCE: uart::Uart = unsafe { uart::Uart::new(0x1000_0000) };
static mut UART_INSTANCE: Option<Uart> = None;

fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
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

#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    clear_bss();
    uart_init();
    mm::init();
    // trap::init();
    // println!("Hello, world!");
    // while(true){

    // }
    panic!("|program finished|");
}
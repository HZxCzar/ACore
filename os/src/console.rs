use crate::uart::Uart;
use core::fmt::{self, Write};

pub static mut UART: Option<*mut Uart> = None;

pub unsafe fn set_uart() {
    if let Some(ref mut uart) = crate::UART_INSTANCE {
        UART = Some(uart as *mut Uart);
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    unsafe {
        if let Some(uart_ptr) = UART {
            let uart = &mut *uart_ptr;
            uart.write_fmt(args).unwrap();
        }
    }
}

pub fn getchar() -> u8 {
    unsafe {
        if let Some(uart_ptr) = UART {
            let uart = &mut *uart_ptr;
            uart.receive()
        } else {
            // 如果UART未初始化，返回0
            0
        }
    }
}


#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

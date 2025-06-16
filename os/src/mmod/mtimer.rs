//! RISC-V timer-related functionality
use crate::board::CLOCK_FREQ;
// use riscv::register::time;

const MSEC_PER_SEC: usize = 1000;

pub fn get_time() -> usize {
    read_time() as usize
}

pub fn get_time_ms() -> usize {
    // println!("get_time_ms: {}", read_time());
    read_time() as usize / (CLOCK_FREQ / MSEC_PER_SEC)
}

pub fn set_next_trigger() {
    set_timer();
}
pub fn read_time() -> u64 {
    const MTIME_ADDR: usize = 0x0200bff8;
    unsafe { (MTIME_ADDR as *const u64).read_volatile() }
}

/// 配置时钟中断
pub fn set_timer() {
    const MTIME_ADDR: usize = 0x0200bff8;
    const MTIMECMP_ADDR: usize = 0x02004000;
    const INTERVAL: u64 = 1000000; // 大约1秒

    unsafe {
        // 读取当前时间
        let mtime = (MTIME_ADDR as *const u64).read_volatile();

        // 设置下一次中断时间
        let next_time = mtime + INTERVAL;
        (MTIMECMP_ADDR as *mut u64).write_volatile(next_time);
    }
}
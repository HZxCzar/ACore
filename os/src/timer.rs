//! RISC-V timer-related functionality
use crate::mmod::{set_timer,read_time};

const TICKS_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1000;

pub fn get_time() -> usize {
    read_time() as usize
}

/// set the next timer interrupt
pub fn set_next_trigger() {
    set_timer();
}

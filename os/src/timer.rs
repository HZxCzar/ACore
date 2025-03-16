//! RISC-V timer-related functionality
use crate::{board::CLOCK_FREQ, mmod::{read_time, set_timer}};

const MSEC_PER_SEC: usize = 1000;

pub fn get_time() -> usize {
    read_time() as usize
}

pub fn get_time_ms() -> usize {
    read_time() as usize / (CLOCK_FREQ / MSEC_PER_SEC)
}

/// set the next timer interrupt
pub fn set_next_trigger() {
    set_timer();
}

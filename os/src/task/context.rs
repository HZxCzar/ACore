//! Implementation of [`TaskContext`]
use crate::trap::trap_return_s;

#[repr(C)]
pub struct TaskContext {
    ra: usize,
    sp: usize,
    s: [usize; 12],
}

impl TaskContext {
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
    pub fn goto_trap_return_s(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return_s as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
    // pub fn get_ra(&self) -> usize {
    //     self.ra
    // }
}

//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.

use super::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::*;
use alloc::collections::BTreeMap;

lazy_static! {
    // 全局引用计数表
    static ref FRAME_REF_COUNT: UPSafeCell<BTreeMap<usize, usize>> = 
        unsafe { UPSafeCell::new(BTreeMap::new()) };
}

pub fn only_one_frame(ppn: PhysPageNum) -> bool {
    let ref_counts = FRAME_REF_COUNT.exclusive_access();
    if let Some(count) = ref_counts.get(&ppn.0) {
        *count == 1
    } else {
        false
    }
}

// 增加引用计数
pub fn increase_frame_ref(ppn: PhysPageNum) {
    let mut ref_counts = FRAME_REF_COUNT.exclusive_access();
    *ref_counts.entry(ppn.0).or_insert(0) += 1;
}

// 减少引用计数，返回是否应该释放
pub fn decrease_frame_ref(ppn: PhysPageNum) -> bool {
    let mut ref_counts = FRAME_REF_COUNT.exclusive_access();
    if let Some(count) = ref_counts.get_mut(&ppn.0) {
        *count -= 1;
        if *count == 0 {
            ref_counts.remove(&ppn.0);
            return true; // 可以释放
        }
    }
    false // 不应该释放
}

/// manage a frame
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl Clone for FrameTracker {
    fn clone(&self) -> Self {
        increase_frame_ref(self.ppn);
        Self { ppn: self.ppn }
    }
}

impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        increase_frame_ref(ppn);
        Self { ppn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        // 只有当引用计数为0时才真正释放帧
        if decrease_frame_ref(self.ppn) {
            frame_dealloc(self.ppn);
        }
    }
}

trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

pub struct StackFrameAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
}
impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }
    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else if self.current == self.end {
            None
        } else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // validity check
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // recycle
        self.recycled.push(ppn);
    }
}

type FrameAllocatorImpl = StackFrameAllocator;

lazy_static! {
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImpl> =
        unsafe { UPSafeCell::new(FrameAllocatorImpl::new()) };
}

pub fn init_frame_allocator() {
    unsafe extern "C" {
        safe fn ekernel();
    }
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(MEMORY_END).floor(),
    );
}

pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc()
        .map(FrameTracker::new)
}

pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

#[allow(unused)]
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    println!("frame_allocator_test passed!");
}

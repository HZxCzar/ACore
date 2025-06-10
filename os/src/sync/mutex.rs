use crate::mm::linked_list::LinkedList;
use alloc::alloc::Layout;
use core::alloc::GlobalAlloc;
use core::cmp::{max, min};
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::UnsafeCell;

const BUDDY_ALLOCATOR_LEVEL: usize = 32;

pub struct SpinMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinMutexGuard<T> {
        while self.locked.compare_and_swap(false, true, Ordering::Acquire) != false {
            core::hint::spin_loop();
        }
        
        SpinMutexGuard { mutex: self }
    }
}

pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<'a, T> core::ops::Deref for SpinMutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}

pub struct LockedHeap {
    pub allocator: SpinMutex<Heap>,
}

pub struct Heap {
    free_lists: [LinkedList; BUDDY_ALLOCATOR_LEVEL],
    gran: usize,
    user: usize,
    allocated: usize,
    total: usize,
}

impl Heap {
    pub const fn new() -> Self {
        Heap {
            free_lists: [LinkedList::new(); BUDDY_ALLOCATOR_LEVEL],
            gran: size_of::<usize>(),
            user: 0,
            allocated: 0,
            total: 0,
        }
    }

    pub const fn empty() -> Self {
        Self {
            free_lists: [LinkedList::new(); BUDDY_ALLOCATOR_LEVEL],
            gran: size_of::<usize>(),
            user: 0,
            allocated: 0,
            total: 0,
        }
    }

    pub unsafe fn add_segment(&mut self, mut start: usize, mut end: usize) {
        start = (start + self.gran - 1) & (!self.gran + 1);
        end = end & (!self.gran + 1);
        self.total += end - start;

        while start < end {
            let level = (end - start).trailing_zeros() as usize;
            self.free_lists[level].push_front(start as *mut usize);
            start += 1 << level;
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let size = self.calculate_size(&layout);
        let level = size.trailing_zeros() as usize;
        for i in level..self.free_lists.len() {
            if !self.free_lists[i].is_empty() {
                // split or no split to find a proper piece
                self.split(level, i);
                let result = self.free_lists[level]
                    .pop_front()
                    .expect("[buddy_allocator] Expect non-empty free list.");

                self.user += layout.size();
                self.allocated += size;
                return result as *mut u8;
            }
        }
        panic!(
            "[buddy_allocator] Unable to allocate more space for size {}.",
            size
        );
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let size = self.calculate_size(&layout);
        let level = size.trailing_zeros() as usize;
        self.merge(level, ptr);
    }

    fn split(&mut self, start: usize, end: usize) {
        for i in (start..end).rev() {
            let ptr = self.free_lists[i + 1]
                .pop_front()
                .expect("[buddy_allocator] Expect non-empty free list.");
            unsafe {
                self.free_lists[i].push_front((ptr as usize + (1 << i)) as *mut usize);
                self.free_lists[i].push_front(ptr);
            }
        }
    }

    fn merge(&mut self, start: usize, ptr: *mut u8) {
        let mut curr = ptr as usize;
        for i in start..self.free_lists.len() {
            let buddy = curr ^ (1 << i);
            let target = self.free_lists[i]
                .iter_mut()
                .find(|node| node.as_ptr() as usize == buddy);

            if let Some(node) = target {
                node.pop();
                curr = min(curr, buddy);
            } else {
                unsafe {
                    self.free_lists[i].push_front(curr as *mut usize);
                }
                break;
            }
        }
    }

    fn calculate_size(&self, layout: &Layout) -> usize {
        return max(
            layout.size().next_power_of_two(),
            max(layout.align(), self.gran),
        );
    }
}

impl LockedHeap {
    pub const fn empty() -> Self {
        Self {
            allocator: SpinMutex::new(Heap::empty()),
        }
    }

    pub unsafe fn init(&self, start: usize, size: usize) {
        self.allocator.lock().add_segment(start, start+size);
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.allocator.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe {
            self.allocator.lock().dealloc(ptr, layout);
        }
    }
}
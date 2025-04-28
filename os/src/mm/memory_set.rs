//! Implementation of [`MapArea`] and [`MemorySet`].

use super::frame_allocator::only_one_frame;
use super::{FrameTracker, frame_alloc};
use super::{PTEFlags, PageTable, PageTableEntry};
use super::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use super::{StepByOne, VPNRange};
use crate::config::{MEMORY_END, MMIO, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT, USER_STACK_SIZE};
use crate::sync::UPSafeCell;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::iter::Map;
use lazy_static::*;
use riscv::paging::PTE;
use riscv::register::satp;

unsafe extern "C" {
    safe fn stext();
    safe fn etext();
    safe fn srodata();
    safe fn erodata();
    safe fn sdata();
    safe fn edata();
    safe fn sbss_with_stack();
    safe fn ebss();
    safe fn ekernel();
    safe fn strampoline();
}

lazy_static! {
    /// a memory set instance through lazy_static! managing kernel space
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> =
        Arc::new(unsafe { UPSafeCell::new(MemorySet::new_kernel()) });
}
/// memory set structure, controls virtual-memory space
pub struct MemorySet {
    page_table: PageTable,
    areas: Vec<MapArea>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }
    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    /// Assume that no conflicts.
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }

    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some((idx, area)) = self
            .areas
            .iter_mut()
            .enumerate()
            .find(|(_, area)| area.vpn_range.get_start() == start_vpn)
        {
            area.unmap(&mut self.page_table);
            self.areas.remove(idx);
        }
    }
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&self.page_table, data);
        }
        self.areas.push(map_area);
    }
    /// Mention that trampoline is not collected by areas.
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
        // unsafe {
        //     println!("TRAMPOLINE虚拟地址: {:#x}", TRAMPOLINE);
        //     println!("TRAMPOLINE物理地址: {:#x}", strampoline as usize);
        // }
    }
    /// Without kernel stacks.
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map kernel sections
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );

        println!("mapping UART section");
        // 添加UART设备映射 - 将物理地址映射到相同的虚拟地址或特定虚拟地址
        memory_set.push(
            MapArea::new(
                VirtAddr::from(0x10000000), // UART起始虚拟地址
                VirtAddr::from(0x10000100), // UART结束虚拟地址
                MapType::Identical,         // 或使用特定的虚拟地址
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping Timer section");
        // 添加Timer设备映射 - 将物理地址映射到相同的虚拟地址或特定虚拟地址
        memory_set.push(
            MapArea::new(
                VirtAddr::from(0x0200b000), // Timer起始虚拟地址
                VirtAddr::from(0x0200c000), // Timer结束虚拟地址
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        memory_set.push(
            MapArea::new(
                VirtAddr::from(0x02004000), // MTIMECMP起始地址
                VirtAddr::from(0x02005000), // MTIMECMP结束地址
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping .text section");
        memory_set.push(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        println!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        println!("mapping .data section");
        memory_set.push(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping memory-mapped registers");
        for pair in MMIO {
            memory_set.push(
                MapArea::new(
                    (*pair).0.into(),
                    ((*pair).0 + (*pair).1).into(),
                    MapType::Identical,
                    MapPermission::R | MapPermission::W,
                ),
                None,
            );
        }
        memory_set
    }
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // guard page
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(
            MapArea::new(
                user_stack_bottom.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // map TrapContext
        memory_set.push(
            MapArea::new(
                TRAP_CONTEXT.into(),
                TRAMPOLINE.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        (
            memory_set,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
        )
    }

    /// fork 时调用：新的 MemorySet
    pub fn from_cow(user_space: &mut Self) -> Self {
        // 1. 新建一张空页表
        let mut child = Self::new_bare();
        child.map_trampoline();
        // println!("TRAP_CONTEXT: {:#x}", VirtAddr::from(TRAP_CONTEXT).0);
        // 2. 遍历父进程每一个 MapArea
        for area in user_space.areas.iter_mut() {
            // println!("area map permission: {:#x}", area.map_perm.bits());
            area.map_type = MapType::Cow;
            let mut haswrite = false;
            if area.map_perm.contains(MapPermission::W) {
                area.map_perm.remove(MapPermission::W);
                haswrite = true;
            }
            let mut new_area = MapArea::from_another(area);
            // area.map(&mut user_space.page_table);

            // 3. 把这个 area 里每一页都取出父 PTE，清掉 WRITE、加上 COW，
            //    然后重新装回父表，并装到子表里
            for vpn in area.vpn_range {
                if vpn == VirtPageNum::from(VirtAddr::from(TRAP_CONTEXT).0 / PAGE_SIZE) {
                    // println!("mapping TRAP_CONTEXT");
                    new_area.map_perm.insert(MapPermission::W);
                    new_area.map_one(&mut child.page_table, vpn, None);
                    let src_ppn = user_space.page_table.translate(vpn).unwrap().ppn();
                    let dst_ppn = child.translate(vpn).unwrap().ppn();
                    dst_ppn
                        .get_bytes_array()
                        .copy_from_slice(src_ppn.get_bytes_array());
                    // println!("vpn: {:#x}, ppn: {:#x}", vpn.0, dst_ppn.0);
                    // println!("child area map permission: {:#x}", new_area.map_perm.bits());
                    new_area.map_perm.remove(MapPermission::W);
                    continue;
                }
                let ppn = user_space.page_table.translate(vpn).unwrap().ppn();
                // println!("parent: vpn: {:#x}, ppn: {:#x}", vpn.0, ppn.0);

                // 父表去掉写权限（Dirty/D bit 可以不动，也可以清）
                let pte_flags = PTEFlags::from_bits(area.map_perm.bits()).unwrap();
                // area.map_perm 已经去掉了 W，所以这里同样不带 W
                user_space.page_table.map_modify(vpn, ppn, pte_flags);

                // 在 child 上用相同的 ppn＋flags 来做只读映射
                let frame_ref = area.data_frames.get(&vpn).unwrap();
                new_area.map_one(&mut child.page_table, vpn, Some(frame_ref.clone()));
                // println!("vpn: {:#x}, ppn: {:#x}", vpn.0, ppn.0);
                // println!("child area map permission: {:#x}\n", new_area.map_perm.bits());
            }

            // 6. 子进程的 MapArea 里也记下它属于 COW 类型
            if (haswrite) {
                area.map_perm.insert(MapPermission::W);
                new_area.map_perm.insert(MapPermission::W);
            }
            child.areas.push(new_area);
        }

        unsafe {
            asm!("sfence.vma");
        }
        // unsafe {
        //     satp::write(child.page_table.token());
        //     asm!("sfence.vma");
        // }
        // let ptr = 0x12345678 as *const u8;
        // println!("byte at 0x12345678: {:#x}", unsafe { *ptr });
        child
    }

    pub fn cow_judge(user_space: &mut Self, fault_addr: VirtAddr) -> bool {
        for area in user_space.areas.iter() {
            let vpn = fault_addr.floor();
            if vpn >= area.vpn_range.get_start() && vpn < area.vpn_range.get_end() {
                if area.map_type == MapType::Cow && area.map_perm.contains(MapPermission::W) {
                    return true;
                }
                return false;
            }
        }
        false
    }

    pub fn cow(&mut self, fault_addr: VirtAddr) -> bool {
        let vpn = fault_addr.floor();
        for area in self.areas.iter_mut() {
            if vpn >= area.vpn_range.get_start() && vpn < area.vpn_range.get_end() {
                // 拿到原 PTE
                // println!(
                //     "vpn: {:#x}, ppn: {:#x}",
                //     vpn.0,
                //     area.vpn_range.get_start().0
                // );
                let old_pte = self.page_table.translate(vpn).unwrap();
                let src_ppn = old_pte.ppn();
                if only_one_frame(src_ppn) {
                    // println!("only one frame");
                    // 直接修改原 PTE
                    let pte_flags = PTEFlags::from_bits(area.map_perm.bits()).unwrap();
                    self.page_table.map_modify(vpn, src_ppn, pte_flags);
                    // println!("vpn: {:#x}, ppn: {:#x}", vpn.0, src_ppn.0);
                } else {
                    let frame = frame_alloc().unwrap();
                    // println!("frame ppn: {:#x}", frame.ppn.0);
                    let dst_ppn = frame.ppn;
                    dst_ppn
                        .get_bytes_array()
                        .copy_from_slice(src_ppn.get_bytes_array());
                    // println!(
                    //     "vpn: {:#x}, ppn: {:#x}",
                    //     vpn.0,
                    //     area.vpn_range.get_start().0
                    // );
                    area.unmap_one(&mut self.page_table, vpn);
                    // 手动设置PTE，不通过map_one重新创建FrameTracker
                    area.data_frames.insert(vpn, frame);
                    let pte_flags = PTEFlags::from_bits(area.map_perm.bits()).unwrap();
                    self.page_table.map(vpn, dst_ppn, pte_flags);
                }
                // 手动插入新的FrameTracker
                // area.map_one(&mut self.page_table, vpn, Some(dst_ppn));
                // let dst_ppn = self.page_table.translate(vpn).unwrap().ppn();
                // dst_ppn
                //     .get_bytes_array()
                //     .copy_from_slice(src_ppn.get_bytes_array());
                unsafe {
                    asm!("sfence.vma");
                }
                return true;
            }
        }
        return false;
    }

    pub fn from_existed_user(user_space: &Self) -> Self {
        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();
        for area in user_space.areas.iter() {
            let new_area = MapArea::from_another(area);
            memory_set.push(new_area, None);
            for vpn in area.vpn_range {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn
                    .get_bytes_array()
                    .copy_from_slice(src_ppn.get_bytes_array());
                // println!("vpn: {:#x}, ppn: {:#x}", vpn.0, dst_ppn.0);
                // println!("child area map permission: {:#x}", area.map_perm.bits());
            }
        }
        memory_set
    }
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            asm!("sfence.vma");
        }
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }

    pub fn recycle_data_pages(&mut self) {
        //*self = Self::new_bare();
        self.areas.clear();
    }
    #[allow(unused)]
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }
    #[allow(unused)]
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.append_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }
}

/// map area structure, controls a contiguous piece of virtual memory
pub struct MapArea {
    vpn_range: VPNRange,
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    map_type: MapType,
    map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    pub fn from_another(another: &Self) -> Self {
        Self {
            vpn_range: VPNRange::new(another.vpn_range.get_start(), another.vpn_range.get_end()),
            data_frames: BTreeMap::new(),
            map_type: another.map_type,
            map_perm: another.map_perm,
        }
    }
    pub fn map_one(
        &mut self,
        page_table: &mut PageTable,
        vpn: VirtPageNum,
        cow_frame: Option<FrameTracker>,
    ) {
        // println!("mapping one");
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
            MapType::Cow => {
                //error
                if let Some(frame) = cow_frame {
                    // println!("mapping cow");
                    //初始化
                    // let frame = frame_alloc().unwrap();
                    // ppn = frame.ppn;
                    ppn = frame.ppn;
                    self.data_frames.insert(vpn, frame);
                } else {
                    //cow handle & TrapContext
                    // println!("mapping cow allocate");
                    let frame = frame_alloc().unwrap();
                    ppn = frame.ppn;
                    self.data_frames.insert(vpn, frame);
                }
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits()).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }
    #[allow(unused)]
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed || self.map_type == MapType::Cow {
            // if self.map_type == MapType::Cow {
            //     println!("unmapping {}", vpn.0);
            // }
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn, None);
        }
    }
    #[allow(unused)]
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    #[allow(unused)]
    pub fn shrink_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(new_end, self.vpn_range.get_end()) {
            self.unmap_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    #[allow(unused)]
    pub fn append_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn, None);
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// map type for memory set: identical or framed
pub enum MapType {
    Identical,
    Framed,
    Cow,
}

bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    #[derive(Copy, Clone)]
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

#[allow(unused)]
pub fn remap_test() {
    let mut kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert!(
        !kernel_space
            .page_table
            .translate(mid_text.floor())
            .unwrap()
            .writable(),
    );
    assert!(
        !kernel_space
            .page_table
            .translate(mid_rodata.floor())
            .unwrap()
            .writable(),
    );
    assert!(
        !kernel_space
            .page_table
            .translate(mid_data.floor())
            .unwrap()
            .executable(),
    );
    println!("remap_test passed!");
}

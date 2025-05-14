use super::{get_block_cache, BlockDevice, BLOCK_SZ};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter, Result};

/// Magic number for sanity check
const EFS_MAGIC: u32 = 0x3b800001;
/// The max number of direct inodes
const INODE_DIRECT_COUNT: usize = 28;
/// The max length of inode name
const NAME_LENGTH_LIMIT: usize = 27;
/// The max number of indirect1 inodes
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
/// The max number of indirect2 inodes
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * INODE_INDIRECT1_COUNT;
/// The upper bound of direct inode index
const DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
/// The upper bound of indirect1 inode index
const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
/// The upper bound of indirect2 inode indexs
#[allow(unused)]
const INDIRECT2_BOUND: usize = INDIRECT1_BOUND + INODE_INDIRECT2_COUNT;
/// Super block of a filesystem
pub struct SuperBlock {
    magic: u32,
    pub total_blocks: u32,
    pub inode_bitmap_blocks: u32,
    pub inode_area_blocks: u32,
    pub data_bitmap_blocks: u32,
    pub data_area_blocks: u32,
}

impl Debug for SuperBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("SuperBlock")
            .field("total_blocks", &self.total_blocks)
            .field("inode_bitmap_blocks", &self.inode_bitmap_blocks)
            .field("inode_area_blocks", &self.inode_area_blocks)
            .field("data_bitmap_blocks", &self.data_bitmap_blocks)
            .field("data_area_blocks", &self.data_area_blocks)
            .finish()
    }
}

impl SuperBlock {
    /// Initialize a super block
    pub fn initialize(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        }
    }
    /// Check if a super block is valid using efs magic
    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}

/// Type of a disk inode
#[derive(PartialEq)]
pub enum DiskInodeType {
    File,
    Directory,
}

/// A indirect block
type IndirectBlock = [u32; BLOCK_SZ / 4];
/// A data block
type DataBlock = [u8; BLOCK_SZ];
/// A disk inode
#[repr(C)]
pub struct DiskInode {
    pub size: u32,
    pub direct: [u32; INODE_DIRECT_COUNT],
    pub indirect1: u32,
    pub indirect2: u32,
    type_: DiskInodeType,
}

impl DiskInode {
    /// Initialize a disk inode, as well as all direct inodes under it
    /// indirect1 and indirect2 block are allocated only when they are needed
    pub fn initialize(&mut self, type_: DiskInodeType) {
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.type_ = type_;
    }
    /// Whether this inode is a directory
    #[allow(unused)]
    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }
    /// Whether this inode is a file
    #[allow(unused)]
    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }
    fn _data_blocks(size: u32) -> u32 {
        (size + BLOCK_SZ as u32 - 1) / BLOCK_SZ as u32
    }
    /// Return block number correspond to size.
    pub fn data_blocks(&self) -> u32 {
        Self::_data_blocks(self.size)
    }
    /// Return number of blocks needed include indirect1/2.
    pub fn total_blocks(size: u32) -> u32 {
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total_blocks = data_blocks as usize;
        if data_blocks > INODE_DIRECT_COUNT {
            total_blocks += 1;
        }
        if data_blocks > INDIRECT1_BOUND {
            total_blocks += 1;
            total_blocks += (data_blocks - INDIRECT1_BOUND + INODE_INDIRECT1_COUNT - 1) / INODE_INDIRECT1_COUNT;
        }
        total_blocks as u32
    }
    /// Get the number of data blocks that have to be allocated given the new size of data
    pub fn blocks_num_needed(&self,new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }
    /// Inncrease the size of current disk inode
    pub fn increase_size(&mut self,new_size: u32,new_blocks: Vec<u32>,block_device: &Arc<dyn BlockDevice>) {
        let mut current_blocks = self.data_blocks();
        self.size = new_size;
        let mut total_blocks = self.data_blocks();
        let mut new_blocks = new_blocks.into_iter();

        while current_blocks<total_blocks.min(INODE_DIRECT_COUNT as u32) {
            self.direct[current_blocks as usize] = new_blocks.next().unwrap();
            current_blocks += 1;
        }

        if total_blocks> INODE_DIRECT_COUNT as u32 {
            if current_blocks == INODE_DIRECT_COUNT as u32 {
                self.indirect1 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_DIRECT_COUNT as u32;
            total_blocks -= INODE_DIRECT_COUNT as u32;
        }
        else{
            return;
        }

        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0,|indirect_block: &mut IndirectBlock| {
                while current_blocks<total_blocks.min(INODE_INDIRECT1_COUNT as u32) {
                    indirect_block[current_blocks as usize] = new_blocks.next().unwrap();
                    current_blocks += 1;
                }
            });

        if total_blocks > INODE_INDIRECT1_COUNT as u32 {
            if current_blocks == INODE_INDIRECT1_COUNT as u32 {
                self.indirect2 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_INDIRECT1_COUNT as u32;
            total_blocks -= INODE_INDIRECT1_COUNT as u32;
        }
        else{
            return;
        }
        
        let mut begin2 = current_blocks as usize / INODE_INDIRECT1_COUNT;
        let mut begin1 = current_blocks as usize % INODE_INDIRECT1_COUNT;
        let end2 = total_blocks as usize / INODE_INDIRECT1_COUNT;
        let end1 = total_blocks as usize % INODE_INDIRECT1_COUNT;
        
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0,|indirect2: &mut IndirectBlock| {
                while (begin2<end2) || (begin2 == end2 && begin1<end1) {
                    if begin1 == 0 {
                        indirect2[begin2] = new_blocks.next().unwrap();
                    }
                    get_block_cache(indirect2[begin2] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0,|indirect1: &mut IndirectBlock| {
                                indirect1[begin1] = new_blocks.next().unwrap();
                        });
                    begin1 += 1;
                    if begin1 == INODE_INDIRECT1_COUNT {
                        begin1 = 0;
                        begin2 += 1;
                    }
                }
            });
    }
    /// Clear size to zero and return blocks that should be deallocated.
    /// We will clear the block contents to zero later.
    pub fn clear_size(&mut self,block_device: &Arc<dyn BlockDevice>) -> Vec<u32>{
        let mut current_blocks = 0usize;
        let mut v:Vec<u32> = Vec::new();
        let mut data_blocks = self.data_blocks() as usize;
        self.size = 0;
        while current_blocks<data_blocks.min(INODE_DIRECT_COUNT) {
            v.push(self.direct[current_blocks]);
            self.direct[current_blocks] = 0;
            current_blocks += 1;
        }

        if data_blocks>INODE_DIRECT_COUNT{
            v.push(self.indirect1);
            data_blocks -= INODE_DIRECT_COUNT;
            current_blocks = 0;
        }
        else{
            return v;
        }
        
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0,|indirect_block: &mut IndirectBlock| {
                while current_blocks<data_blocks.min(INODE_INDIRECT1_COUNT) {
                    v.push(indirect_block[current_blocks]);
                    // indirect_block[current_blocks] = 0; ???
                    current_blocks += 1;
                }
            });
        
        self.indirect1 = 0;

        if data_blocks>INODE_INDIRECT1_COUNT{
            v.push(self.indirect2);
            data_blocks -= INODE_INDIRECT1_COUNT;
            current_blocks = 0;
        }
        else{
            return v;
        }

        assert!(data_blocks <= INODE_INDIRECT2_COUNT);
        let ind2 = data_blocks / INODE_INDIRECT1_COUNT;
        let ind1 = data_blocks % INODE_INDIRECT1_COUNT;
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0,|indirect2: &mut IndirectBlock| {
                for entry in indirect2.iter_mut().take(ind2) {
                    v.push(*entry);
                    get_block_cache(*entry as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter() {
                                v.push(*entry);
                            }
                        });
                }

                if ind1 > 0 {
                    v.push(indirect2[ind2]);
                    get_block_cache(indirect2[ind2] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter().take(ind1) {
                                v.push(*entry);
                            }
                        });
                }
            });
        self.indirect2 = 0;
        v
    }
    /// Get id of block given inner id
    pub fn get_block_id(&self,inner_id:u32,block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            self.direct[inner_id]
        } else if inner_id < INDIRECT1_BOUND {
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .read(0,|indirect_block: &IndirectBlock| {indirect_block[inner_id - INODE_DIRECT_COUNT]})
        } else {
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                .lock()
                .read(0,|indirect2: &IndirectBlock| {indirect2[last / INODE_INDIRECT1_COUNT]});
            get_block_cache(indirect1 as usize, Arc::clone(block_device))
            .lock()
            .read(0,|indirect1: &IndirectBlock| {indirect1[last % INODE_INDIRECT1_COUNT]})
        }
    }
    /// Read data from current disk inode
    pub fn read_at(&self,
        offset: usize,
        buf: &mut [u8],
        block_device: &Arc<dyn BlockDevice>)
        -> usize {
            let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        if start >= end {
            return 0;
        }
        let mut start_block = start / BLOCK_SZ;
        let mut read_size = 0usize;
        loop{
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            if end_current_block > end {
                end_current_block = end;
            }
            let block_read_size = end_current_block - start;
            let dst = &mut buf[read_size..read_size + block_read_size];
            get_block_cache(self.get_block_id(start_block as u32, block_device) as usize, Arc::clone(block_device))
                .lock()
                .read(0, |data_block: &DataBlock| {
                    dst.copy_from_slice(&data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_read_size]);
                });
            read_size += block_read_size;
            // move to next block
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        read_size
        }
    /// Write data into current disk inode
    /// size must be adjusted properly beforehand
    pub fn write_at(&mut self,
        offset: usize,
        buf: &[u8],
        block_device: &Arc<dyn BlockDevice>)->usize{
            let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        assert!(start <= end);
        if start >= end {
            return 0;
        }
        let mut start_block = start / BLOCK_SZ;
        let mut write_size = 0usize;
        loop{
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            if end_current_block > end {
                end_current_block = end;
            }
            let block_write_size = end_current_block - start;
            get_block_cache(self.get_block_id(start_block as u32, block_device) as usize, Arc::clone(block_device))
                .lock()
                .modify(0, |data_block: &mut DataBlock| {
                    let dst = &mut data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_write_size];
                    dst.copy_from_slice(&buf[write_size..write_size + block_write_size]);
                });
            write_size += block_write_size;
            // move to next block
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        write_size
        }
}


/// A directory entry
#[repr(C)]
pub struct DirEntry {
    name: [u8; NAME_LENGTH_LIMIT + 1],
    inode_number: u32,
}
/// Size of a directory entry
pub const DIRENT_SZ: usize = 32;

impl DirEntry{
    /// Crate a directory entry from name and inode number
    pub fn new(name: &str, inode_number: u32) -> Self {
        let mut bytes = [0u8;NAME_LENGTH_LIMIT + 1];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Self {
            name: bytes,
            inode_number,
        }
    }
    /// Create an empty directory entry
    pub fn empty() -> Self {
        Self {
            name: [0u8; NAME_LENGTH_LIMIT + 1],
            inode_number: 0,
        }
    }
    /// Serialize into bytes
    pub fn as_bytes(&self)->&[u8]{
        unsafe{ core::slice::from_raw_parts(self as *const _ as usize as *const u8, DIRENT_SZ) }
    }
    /// Serialize into mutable bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as usize as *mut u8, DIRENT_SZ) }
    }
    /// Get name of the entry
    pub fn name(&self) -> &str {
        let len = (0usize..).find(|i| self.name[*i] == 0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }
    /// Get inode number of the entry
    pub fn inode_number(&self) -> u32 {
        self.inode_number
    }
}


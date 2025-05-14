//! `Arc<Inode>` -> `OSInodeInner`: In order to open files concurrently
//! we need to wrap `Inode` into `Arc`,but `Mutex` in `Inode` prevents
//! file systems from being accessed simultaneously
//!
//! `UPSafeCell<OSInodeInner>` -> `OSInode`: for static `ROOT_INODE`,we
//! need to wrap `OSInodeInner` into `UPSafeCell`
use super::File;
use crate::drivers::BLOCK_DEVICE;
use crate::mm::UserBuffer;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use core::any::Any;
use easy_fs::{EasyFileSystem, Inode};
use lazy_static::*;
/// A wrapper around a filesystem inode
/// to implement File trait atop
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}
/// The OS inode inner in 'UPSafeCell'
pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    /// Construct an OS inode from a inode
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }
    /// Read all data inside a inode into vector
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}
/// List all files in the filesystems
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

bitflags! {
    ///Open file flags
    pub struct OpenFlags: u32 {
        ///Read only
        const RDONLY = 0;
        ///Write only
        const WRONLY = 1 << 0;
        ///Read & Write
        const RDWR = 1 << 1;
        ///Allow create
        const CREATE = 1 << 9;
        ///Clear file and return an empty one
        const TRUNC = 1 << 10;
   }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// Return (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

/// find inode by path
fn find_inode(path: &str) -> Option<Arc<Inode>> {
    let mut inode = ROOT_INODE.clone();
    for name in path.split('/') {
        if name.is_empty() {
            continue;
        }
        if let Some(new_inode) = inode.find(name) {
            inode = new_inode;
        } else {
            return None;
        }
    }
    Some(inode)
}

///Open file with flags
pub fn open_file(path: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        // 拆分路径：parent_path + name
        let (parent_path, name) = if let Some(pos) = path.rfind('/') {
            let parent = &path[..pos];
            let name = &path[pos + 1..];
            (parent, name)
        } else {
            ("", path)
        };
        if name.is_empty() {
            return None;
        }
        if let Some(inode) = find_inode(path) {
            // exist, clear it
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else if let Some(parent_inode) = find_inode(parent_path) {
            parent_inode
                .create_file(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            None
        }
    } else {
        find_inode(path).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

pub fn delete_file(path: &str) -> bool {
    let (parent_path, name) = if let Some(pos) = path.rfind('/') {
        let parent = &path[..pos];
        let name = &path[pos + 1..];
        (parent, name)
    } else {
        ("", path)
    };
    if name.is_empty() {
        return false;
    }
    if let Some(parent_inode) = find_inode(parent_path) {
        parent_inode.delete_entry(name)
    } else {
        false
    }
}

pub fn make_dir(path: &str) -> bool {
    let (parent_path, name) = if let Some(pos) = path.rfind('/') {
        let parent = &path[..pos];
        let name = &path[pos + 1..];
        (parent, name)
    } else {
        ("", path)
    };
    if name.is_empty() {
        return false;
    }
    if let Some(parent_inode) = find_inode(parent_path) {
        parent_inode.create_dir(name).is_some()
    } else {
        false
    }
}

pub fn remove_dir(path: &str) -> bool {
    let (parent_path, name) = if let Some(pos) = path.rfind('/') {
        (&path[..pos], &path[pos + 1..])
    } else {
        ("", path)
    };
    if name.is_empty() {
        return false;
    }
    if let Some(parent_inode) = find_inode(parent_path) {
        if let Some(dir_inode) = parent_inode.find(name) {
            // 1. 判断是不是目录（必须！）
            let is_dir = dir_inode.is_dir();
            if !is_dir {
                return false; // 不是目录，不能用 rmdir
            }
            // 2. 判断目录是否为空
            let file_list = dir_inode.ls();
            if !file_list.is_empty() {
                return false; // 目录非空
            }
            // 3. 执行 unlink
            return parent_inode.delete_entry(name);
        } else {
            return false;
        }
    }
    false
}

/// 移动/重命名文件或目录（不覆盖，不跨文件系统）
/// old_path -> new_path
pub fn rename_file_or_dir(old_path: &str, new_path: &str) -> bool {
    // 路径分割
    let (old_parent_path, old_name) = if let Some(pos) = old_path.rfind('/') {
        (&old_path[..pos], &old_path[pos + 1..])
    } else { ("", old_path) };
    let (new_parent_path, new_name) = if let Some(pos) = new_path.rfind('/') {
        (&new_path[..pos], &new_path[pos + 1..])
    } else { ("", new_path) };
    if old_name.is_empty() || new_name.is_empty() { return false; } 

    let old_parent_inode = match find_inode(old_parent_path) { Some(x) => x, None => return false };
    let moved_id = match old_parent_inode.find_id(old_name) { Some(x) => x, None => return false };
    let new_parent_inode = match find_inode(new_parent_path) { Some(x) => x, None => return false };
    // 新名或同路径已存在
    if new_parent_inode.find(new_name).is_some() { return false; }

    // 新目录加entry指向同 inode_id
    if !new_parent_inode.link(new_name, moved_id) { return false; }

    // 原目录删原entry
    if !old_parent_inode.unlink_entry(old_name) { return false; }
    
    // for name in new_parent_inode.ls() {
    //     println!("{}", name);
    // }

    true
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }
    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }

    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
    fn seek(&self, offset: isize, whence: usize) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        // 获取文件长度
        let file_size = inner.inode.size();

        let new_offset = match whence {
            0 /* SEEK_SET */ => {
                if offset < 0 {
                    return None;
                }
                offset as usize
            },
            1 /* SEEK_CUR */ => {
                let tmp = inner.offset as isize + offset;
                if tmp < 0 {
                    return None;
                }
                tmp as usize
            },
            2 /* SEEK_END */ => {
                let tmp = file_size as isize + offset;
                if tmp < 0 || tmp as usize > file_size {
                    return None;
                }
                tmp as usize
            },
            _ => return None,
        };
        inner.offset = new_offset;
        Some(new_offset)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

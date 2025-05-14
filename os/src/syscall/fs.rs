//! File and filesystem-related syscalls
use crate::fs::{OpenFlags, open_file, delete_file, make_dir, remove_dir, rename_file_or_dir};
use crate::mm::{UserBuffer, translated_byte_buffer, translated_str};
use crate::task::{current_task, current_user_token};

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_delete(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if delete_file(path.as_str()) {
        0
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

pub fn sys_lseek(fd: usize, offset: isize, whence: usize) -> isize {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();

    if fd >= inner.fd_table.len() {
        return -1;
    }
    let file = match &inner.fd_table[fd] {
        Some(file) => file.clone(),
        None => return -1,
    };
    file.seek(offset, whence)
        .map(|new_offset| new_offset as isize)
        .unwrap_or(-1)
}

pub fn sys_mkdir(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if make_dir(path.as_str()) {
        0
    } else {
        -1
    }
}

pub fn sys_rmdir(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if remove_dir(path.as_str()) {
        0
    } else {
        -1
    }
}

pub fn sys_rename(old_path: *const u8, new_path: *const u8) -> isize {
    let token = current_user_token();
    let old_path = translated_str(token, old_path);
    let new_path = translated_str(token, new_path);
    if rename_file_or_dir(old_path.as_str(), new_path.as_str()) {
        0
    } else {
        -1
    }
}
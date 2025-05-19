//! File system in os
mod inode;
mod stdio;
mod pipe;

use crate::mm::UserBuffer;
/// File trait
use core::any::Any;

pub trait File: Send + Sync {
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
    fn read(&self, buf: UserBuffer) -> usize;
    fn write(&self, buf: UserBuffer) -> usize;
    fn seek(&self, offset: isize, whence: usize) -> Option<usize>;
    fn as_any(&self) -> &dyn Any;
}

pub use inode::{OSInode, OpenFlags, list_apps, open_file, delete_file, make_dir, remove_dir, rename_file_or_dir};
pub use stdio::{Stdin, Stdout};
pub use pipe::make_pipe;

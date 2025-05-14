#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{OpenFlags, close, open, read, write, mkdir, rmdir, delete, lseek, rename};

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    // 1. 创建目录和文件
    let dir_name = "testdir\0";
    assert_eq!(mkdir(dir_name), 0);   // 创建新目录
    assert_eq!(mkdir(dir_name), -1);  // 重复失败

    let file_path = "testdir/filex\0";
    let test_str = "Rust: hello, directories!";
    let fd = open(file_path, OpenFlags::CREATE | OpenFlags::WRONLY);
    assert!(fd > 0);
    let fd = fd as usize;
    write(fd, test_str.as_bytes());
    close(fd);

    // 2. 文件内容校验
    let fd = open(file_path, OpenFlags::RDONLY);
    assert!(fd > 0);
    let mut buffer = [0u8; 100];
    assert_eq!(lseek(fd as usize, 0, 0), 0);
    let read_len = read(fd as usize, &mut buffer) as usize;
    assert_eq!(test_str, core::str::from_utf8(&buffer[..read_len]).unwrap());
    assert_eq!(lseek(fd as usize, 6, 0), 6);
    let read_tail = read(fd as usize, &mut buffer[0..]) as usize;
    assert_eq!(&test_str[6..], core::str::from_utf8(&buffer[..read_tail]).unwrap());
    close(fd as usize);

    // 3. 重命名文件: 同目录下
    let new_file = "testdir/filey\0";
    assert_eq!(rename(file_path, new_file), 0);
    // 旧文件应打不开
    assert!(open(file_path, OpenFlags::RDONLY) < 0);
    // 新文件应能读
    let fd = open(new_file, OpenFlags::RDONLY);
    assert!(fd > 0);
    let read_len = read(fd as usize, &mut buffer) as usize;
    assert_eq!(test_str, core::str::from_utf8(&buffer[..read_len]).unwrap());
    close(fd as usize);

    // 4. 创建新目录
    let dir2 = "testdir2\0";
    assert_eq!(mkdir(dir2), 0);

    // 5. 移动文件到新目录
    let moved_file = "testdir2/filemoved\0";
    assert_eq!(rename(new_file, moved_file), 0);
    // 新路径可打开，内容一致
    let fd = open(moved_file, OpenFlags::RDONLY);
    assert!(fd > 0);
    let read_len = read(fd as usize, &mut buffer) as usize;
    assert_eq!(test_str, core::str::from_utf8(&buffer[..read_len]).unwrap());
    close(fd as usize);

    // 试图覆盖同名（应失败）
    let fd2 = open(moved_file, OpenFlags::CREATE | OpenFlags::WRONLY);
    assert!(fd2 > 0);
    close(fd2 as usize);
    assert_eq!(rename(moved_file, moved_file), -1);

    // 6. 目录重命名（空目录）
    let newdir = "renameddir\0";
    assert_eq!(mkdir("emptydir\0"), 0);
    assert_eq!(rename("emptydir\0", newdir), 0);
    // 旧目录不能删了，因为已经换名字
    assert_eq!(rmdir("emptydir\0"), -1);
    assert_eq!(rmdir(newdir), 0);

    // 7. 非空目录重命名（testdir2)
    let bigdir = "bigdir\0";
    assert_eq!(rename(dir2, bigdir), 0);
    // 文件移动后旧目录消失，新目录可删
    assert_eq!(rmdir(dir2), -1);
    // 但新目录还不能删（有文件）
    assert_eq!(rmdir(bigdir), -1);
    // 删文件再删目录
    assert_eq!(delete("bigdir/filemoved\0"), 0);
    assert_eq!(rmdir(bigdir), 0);

    // 尾部清理
    assert_eq!(rmdir(dir_name), 0); // remove testdir
    assert_eq!(rmdir(dir_name), -1);

    println!("dir/seek/delete/rename_test passed!");
    0
}
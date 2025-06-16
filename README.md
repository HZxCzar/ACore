# ACore
ACore——operating system

## 引导加载程序（Bootloader）

- 初始化Uart
- 为内核进入M模式 /mmod

## 内存分配器（Allocator）

- Buddy allocator
- 帧分配器（Frame allocator） /mm

## 页表（Page table）

- 内核页表
- 每个用户进程的页表
- fork启用COW /memory_set.rs::from_cow

## 控制台（Console）

- 读操作
- 写操作

## 消息与数据传输（Message & data transfer）

- 用户 -> 内核
- 内核 -> 用户
- 内核 -> 内核
- 用户 -> 用户 /syscall

## 进程（Process）

- 进程加载
  - ELF解析 /memory_set.rs::from_elf
  - 调度重新加载 /task/mode.rs::suspend_current_and_run_next
- 系统调用
  - 启动新进程（fork和exec）
  - 等待子进程（wait）
  - 进程退出（exit）/syscall
- 进程管理器
  - 进程创建
  - 进程交互 /syscall->pipe
  - 进程终止 /task/mode.rs::exit_current_and_run_next
- 调度器
  - 上下文切换 /task/switch
  - 调度机制（时间共享）/trap/mod
- 定时器中断 /trap/mod

## 同步原语（Synchronization primitives）

- SpinLock，无多线程 /sync/mutex

## 文件系统（File system）

- 文件/目录创建/删除
- 文件/目录重命名
- 文件读取
- 文件写入
- 文件/目录移动 /fs /easyfs

use core::panic::PanicInfo;
use crate::println;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    shutdown(true)
}

/// 关闭计算机
pub fn shutdown(failure: bool) -> ! {
    // println!("Shutdown machine!");
    
    // QEMU virt 平台的 test 设备地址
    const VIRT_TEST_ADDR: *mut u32 = 0x100000 as *mut u32;
    
    // virt_test 设备的退出代码
    const VIRT_TEST_EXIT_SUCCESS: u32 = 0x5555; // 正常退出
    
    unsafe {
        // 写入退出代码到 test 设备地址，触发 QEMU 退出
        VIRT_TEST_ADDR.write_volatile(VIRT_TEST_EXIT_SUCCESS);
        
        // 无限循环，确保函数不返回
        loop {}
    }
}
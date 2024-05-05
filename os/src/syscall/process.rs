//! Process management syscalls
use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},task::{
        change_program_brk, current_tranlated_physical_address, exit_current_and_run_next, get_current_start_time, get_current_taskcontrolblock_status, get_syscall_times, mmap_current_task, munmap_current_task, suspend_current_and_run_next, TaskStatus
    }, timer::get_time_us
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let _us = get_time_us();
    let ts = current_tranlated_physical_address(_ts as *const u8 ) as *mut TimeVal;
    unsafe {
        *ts = TimeVal{
            sec:_us / 1_000_000,
            usec : _us % 1_000_000,
        }
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    let _ti =  current_tranlated_physical_address(ti as *const u8 ) as *mut TaskInfo;
    unsafe{
        *_ti = TaskInfo{
            status : get_current_taskcontrolblock_status(),
            syscall_times : get_syscall_times(),
            time : (get_time_us() - get_current_start_time()) / 1_000
        }
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    // 首先检查start是否按页对齐
    if start % PAGE_SIZE != 0{
        return -1;
    }
    // 检查其余位必须为0的条件
    if port & !0x7 != 0{
        return -1;
    }
    // 检查以下的内存是否具有意义
    if !port & 0x7 == 7{
        return -1;
    }
    // 通过参数检查 调用实现的mmap方法为其分配空间
    //println!("pass the check\n");
    // 获取内核实例 取得所有权完成分配(这样不对 你并没有找到实际你要分配的位置) 实际上你是给内核多次分配了 所以才这样子报错
    //let num = KERNEL_SPACE.exclusive_access().mmap(start, len, port);

    //问题在于如何获得当前应用空间所在的MemorySet
    //妈的 找到了 在TCB里面有 byd受不了了
    //最好的方式其实还是在TCB里面把地址空间映射了 然后再封装出来
    let num = mmap_current_task(start, len, port);
    //let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
    //let num = map_area.exclusive_access().mmap(start, len, port);
    num
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    if start % PAGE_SIZE != 0{
        return -1;
    }
    let num = munmap_current_task(start, len);
    num
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

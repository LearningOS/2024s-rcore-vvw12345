//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::mm::translated_physical_address;
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }

    ///为Lab5添加 
    ///获得当前任务的start time
    pub fn get_current_tasks_start_time(&self) -> usize{
        let inner = self.current.as_ref().unwrap().inner_exclusive_access();
        inner.start_time
    }

    /// 获得当前任务的系统调用信息
    pub fn get_syscall_times(&self) -> [u32;500]{
        let inner = self.current.as_ref().unwrap().inner_exclusive_access();
        inner.syscall_times
    }

    /// 获得当前任务状态
    pub fn get_current_task_status(&self) -> TaskStatus{
        let inner = self.current.as_ref().unwrap().inner_exclusive_access();
        inner.task_status
    }

    /// 添加系统调用
    pub fn add_current_syscall_times(&mut self,syscall_id:usize){
        let mut current_inner = self.current.as_mut().unwrap().inner_exclusive_access();
        current_inner.syscall_times[syscall_id] += 1;
        //println!("{} + 1\n",syscall_id);
    }

    /// 为当前地址空间完成映射
    pub fn mmap_current_task(&mut self,start: usize,len: usize,port: usize) -> isize{
        let mut current_inner = self.current.as_mut().unwrap().inner_exclusive_access();
        let memory_set = &mut current_inner.memory_set;
        memory_set.mmap(start,len,port)
    }

    /// 为当前地址空间解映射
    pub fn munmap_current_task(&mut self,start: usize,len: usize) -> isize{
        let mut current_inner = self.current.as_mut().unwrap().inner_exclusive_access();
        let memory_set = &mut current_inner.memory_set;
        memory_set.munmap(start,len)
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            if task_inner.start_time == 0{
                task_inner.start_time = get_time_us();
            }
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

/// 获取当前任务的时间
pub fn current_task_start_time() -> usize{
    PROCESSOR.exclusive_access().get_current_tasks_start_time()
}

/// 获取当前任务的系统调用次数
pub fn current_task_syscall_times() -> [u32;500]{
    PROCESSOR.exclusive_access().get_syscall_times()
}

/// 发生系统调用 增加一次次数
pub fn add_syscall_times(syscall_id:usize){
    PROCESSOR.exclusive_access().add_current_syscall_times(syscall_id);
}

/// 获得当前任务的任务状态
pub fn current_task_status() -> TaskStatus{
    PROCESSOR.exclusive_access().get_current_task_status()
}

/// 为当前的任务完成地址空间翻译
pub fn current_tranlated_physical_address(ptr:*const u8) -> usize{
    let token = current_user_token();
    translated_physical_address(token,ptr)
}

/// 为当前的地址空间完成地址映射
pub fn mmap_current_task(start: usize,len: usize,port: usize) -> isize{
    PROCESSOR.exclusive_access().mmap_current_task(start, len, port)
}

/// 为当前地址空间解开地址映射
pub fn munmap_current_task(start: usize,len: usize) -> isize{
    PROCESSOR.exclusive_access().munmap_current_task(start, len)
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

## Lab3

### 本章代码导读

本章的重点是实现对应用之间的协作式和抢占式任务切换的操作系统支持。与上一章的操作系统实现相比，有如下一些不同的情况导致实现上也有差异：

- 多个应用同时放在内存中，所以他们的起始地址是不同的，且地址范围不能重叠
- 应用在整个执行过程中会暂停或被抢占，即会有主动或被动的任务切换

这些实现上差异主要集中在对应用程序执行过程的管理、支持应用程序暂停的系统调用和主动切换应用程序所需的时钟中断机制的管理。

对于第一个不同情况，需要**对应用程序的地址空间布局进行调整，每个应用的地址空间都不相同，且不能重叠。**这并不要修改应用程序本身，而是通过一个脚本 `build.py` 来针对每个应用程序修改链接脚本 `linker.ld` 中的 `BASE_ADDRESS` ，让编译器在编译不同应用时用到的 `BASE_ADDRESS` 都不同，且有足够大的地址间隔。这样就可以让每个应用所在的内存空间是不同的。

对于第二个不同情况，需要**实现任务切换，这就需要在上一章的 Trap 上下文切换的基础上，再加上一个 Task 上下文切换，才能完成完整的任务切换。**这里面的关键数据结构是表示应用执行上下文的 `TaskContext` 数据结构和具体完成上下文切换的汇编语言编写的 `__switch` 函数。一个应用的执行需要被操作系统管理起来，这是通过 `TaskControlBlock` 数据结构来表示应用执行上下文的动态执行过程和状态（运行态、就绪态等）。而为了做好应用程序第一次执行的前期初始化准备， `TaskManager` 数据结构的全局变量实例 `TASK_MANAGER` 描述了应用程序初始化所需的数据， 而对 `TASK_MANAGER` 的初始化赋值过程是实现这个准备的关键步骤。

应用程序可以在用户态执行中**主动暂停**，这需要有新的系统调用 `sys_yield` 的实现来支持；为了支持抢占应用执行的抢占式切换，还要**添加对时钟中断的处理**。有了时钟中断，就可以在确定时间间隔内打断应用的执行，并主动切换到另外一个应用，这部分主要是通过对 `trap_handler` 函数中进行扩展，来完成在时钟中断产生时可能进行的任务切换。 `TaskManager` 数据结构的成员函数 `run_next_task` 来具体实现基于任务控制块的任务切换，并会具体调用 `__switch` 函数完成硬件相关部分的任务上下文切换。

### 多道程序放置和加载——锯齿螈OS

![image-20240426111820382](D:\116\sigs\way2sigs-main\912\一轮复习\rcore_lab.assets\image-20240426111820382.png)

这个OS和前一个章节的BatchOS区别不大，最大的区别在于一次将多个程序加载进内存中。

通过脚本build.py为每一个应用程序定制一个脚本，使得每个应用程序有各自不同的地址。

随后各自的linker.ld将应用加载到内存中。

执行时由linker.ld中提供的符号，OS可以知道每个应用的地址，从而在Trap(执行结束或程序异常)的时候，可以切换到对应地址执行下一个应用，不需要像BatchOS那样将新应用copy到0x80200000再执行。

### 任务切换

任务：**一个具有一定独立功能的程序在一个数据集合上的一次动态执行过程**（好像只有THU有这个概念……~~汗~~)

进程还会对很多资源进行管理……任务不会

任务没有彻底的地址空间隔离……任务之间也没有协同……



#### 不同类型的上下文切换

- 函数调用

  ​	当时提到过，为了支持嵌套函数调用，不仅需要硬件平台提供特殊的跳转指令，还需要保存和恢复函数调用上下文 。注意在上述定义中，函数调用包含在**普通控制流**（与异常控制流相对）之内，且**始终用一个固定的栈来保存执行的历史记录**，因此函数调用并**不涉及控制流的特权级切换**。但是我们依然可以将其看成调用者和被调用者两个执行过程的“切换”，**二者的协作体现在它们都遵循调用规范，分别保存一部分通用寄存器**，这样的好处是编译器能够有足够的信息来尽可能减少需要保存的寄存器的数目。虽然当时用了很大的篇幅来说明，但**其实整个过程都是编译器负责完成的，我们只需设置好栈就行了**。

- Trap(异常)控制流

  ​	需要保存和恢复系统调用（Trap）上下文 。当时，为了让内核能够完全掌控应用的执行，且不会被应用破坏整个系统，我们必须利用硬件提供的特权级机制，让应用和内核运行在不同的特权级。应用运行在 U 特权级，它所被允许的操作进一步受限，处处被内核监督管理；而内核运行在 S 特权级，有能力处理应用执行过程中提出的请求或遇到的状况。

- 任务切换

  ​	任务切换是来自**两个不同应用在内核中的 Trap 控制流之间的切换**。当一个应用 Trap 到 S 模式的操作系统内核中进行进一步处理（即进入了操作系统的 Trap 控制流）的时候，其 Trap 控制流可以调用一个特殊的 `__switch` 函数。这个函数表面上就是一个普通的函数调用：在 `__switch` 返回之后，将继续从调用该函数的位置继续向下执行。但是其间却隐藏着复杂的控制流切换过程。具体来说，调用 `__switch` 之后直到它返回前的这段时间，原 Trap 控制流 *A* 会先被暂停并被切换出去， CPU 转而运行另一个应用在内核中的 Trap 控制流 *B* 。然后在某个合适的时机，原 Trap 控制流 *A* 才会从某一条 Trap 控制流 *C* （很有可能不是它之前切换到的 *B* ）切换回来继续执行并最终返回。不过，从实现的角度讲， `__switch` 函数和一个普通的函数之间的核心差别仅仅是它会 **换栈** 。

![../_images/switch.png](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/switch.png)

以下是__switch的具体实现

```assembly
.altmacro
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch
__switch:
    # __switch(  该符号在RUST中将被解释为一个函数
    #     current_task_cx_ptr: *mut TaskContext,
    #     next_task_cx_ptr: *const TaskContext
    # ) 从RISC-V调用规范可知 current_task_cx_ptr和next_task_cx_ptr分别会通过寄存器a0/a1传入
    # save kernel stack of current task
    sd sp, 8(a0) #保存栈指针到a0打头偏移地址为8的位置
    # save ra & s0~s11 of current execution
    sd ra, 0(a0)
    .set n, 0  #循环变量n=0 通过.rept 12循环SAVE_SN宏 12次 从而保存a0到a11寄存器
    .rept 12  
        SAVE_SN %n
        .set n, n + 1
    .endr
    # restore ra & s0~s11 of next execution
    ld ra, 0(a1) #加载返回地址
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # restore kernel stack of next task
    ld sp, 8(a1) #恢复下一个任务的栈指针 
    ret
```

这里可以看出switch实现和一般的函数调用的区别，一般的函数调用编译器会自动生成代码来保存s0~s11这些通用寄存器，而switch就不会（因为其是汇编语言实现的函数），不会被编译器处理，所以我们需要手动保存这些通用寄存器。

```rust
pub struct TaskContext {
    ra: usize,
    sp: usize,//保存的也是栈指针，但和函数调用的区别在于switch过程中有一次换栈
    s: [usize; 12],
}
```

### 多道程序和协作式调度

![始初龙协作式多道程序操作系统 -- CoopOS总体结构](https://rcore-os.cn/rCore-Tutorial-Book-v3/_images/more-task-multiprog-os-detail.png)

任务控制块

```rust
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    UnInit, // 未初始化
    Ready, // 准备运行
    Running, // 正在运行
    Exited, // 已退出
}

#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    pub task_status: TaskStatus,//当前任务执行状态
    pub task_cx: TaskContext,//任务上下文
}

pub struct TaskManager {
    num_app: usize, //总共管理多少个任务
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],//任务列表
    current_task: usize,//当前执行的任务
}
```

#### 主动放弃yield和退出exit

```rust
//主动放弃
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

//退出
pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

impl TaskManager {
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.borrow_mut();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    fn mark_current_exited(&self) {
        let mut inner = self.inner.borrow_mut();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }
}
```

重点是下面的

```rust
//执行下一个任务
fn run_next_task(&self) {
    if let Some(next) = self.find_next_task() {
        let mut inner = self.inner.exclusive_access();//获得进程控制块的可变借用 从而可以修改
        let current = inner.current_task;
        //println!("task {} start",current);
        inner.tasks[next].task_status = TaskStatus::Running;
        inner.current_task = next;
        let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
        let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
        drop(inner); //前面拿到了inner的可变借用 这里要放弃掉 要不然__switch切换的时候没办法对里面的内容进行修改(一般来说会在函数生命周期结束的时候drop掉)
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(current_task_cx_ptr, next_task_cx_ptr);
        }
        // go back to user mode 
    } else {
        panic!("All applications completed!");
    }
}

//找到下一个任务
fn find_next_task(&self) -> Option<usize> {
    let inner = self.inner.exclusive_access();
    let current = inner.current_task;
    //这里从current+1开始只是一种调度的方式，你从0开始也一样
    //不过current+1开始会更好一点 从0开始碰到Ready的就执行了 后面可能会饥饿
    (current + 1..current + self.num_app + 1) 
        .map(|id| id % self.num_app)
        .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
}
```

#### 第一次进入用户态

和前一个章节的情况是类似的……要把特定任务的任务上下文构造出来，然后压入内核栈顶，然后在switch返回之后restore恢复上下文从而开始执行。

如果是所有程序当中最先执行的那个，需要特殊构造一个unused上下文(后面也不会用到了)来填充switch的参数，主要是为了避免覆盖到其他的数据。

```rust
impl TaskContext {
    pub fn goto_restore(kstack_ptr: usize) -> Self {
        extern "C" { fn __restore(); }
        Self {
            ra: __restore as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}

pub fn init_app_cx(app_id: usize) -> usize {
    KERNEL_STACK[app_id].push_context(//上下文压入内核栈顶
        TrapContext::app_init_context(get_base_i(app_id), USER_STACK[app_id].get_sp()),
    )
}
```

### 分时多任务和抢占式调度

![image-20240426140842737](D:\116\sigs\way2sigs-main\912\一轮复习\rcore_lab.assets\image-20240426140842737.png)

实现的关键是在trap_handler中新增一个分支，可以检测到S特权级的时钟中断

```rust
const SBI_SET_TIMER: usize = 0;

pub fn set_timer(timer: usize) {//设置mtimecmp的值
    //当计时器mtime大于mtimecmp时，就会触发时钟中断
    sbi_call(SBI_SET_TIMER, timer, 0, 0);//基于sbi_call的计时功能
}

// os/src/timer.rs

use crate::config::CLOCK_FREQ;
const TICKS_PER_SEC: usize = 100;

pub fn set_next_trigger() { //设置好下一次时钟中断的时间点
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

match scause.cause() {
    Trap::Interrupt(Interrupt::SupervisorTimer) => {
        set_next_trigger();
        suspend_current_and_run_next();
    }
}
```



### 练习：获取任务信息

#### 需求

ch3 中，我们的系统已经能够支持多个任务分时轮流运行，我们希望引入一个新的系统调用 `sys_task_info` 以获取当前任务的信息，定义如下：

```rust
fn sys_task_info(ti: *mut TaskInfo) -> isize
```

- syscall ID: 410
- 查询当前正在执行的任务信息，任务信息包括任务控制块相关信息（任务状态）、任务使用的系统调用及调用次数、系统调用时刻距离任务第一次被调度时刻的时长（单位ms）。

```rust
struct TaskInfo {
    status: TaskStatus,//任务状态 从TaskControlBlock中可以拿到
    syscall_times: [u32; MAX_SYSCALL_NUM],//任务使用的系统调用及其次数
    time: usize//当前系统调用时刻距离开始时候的时长(就是个计时器)
}
```

#### 简要过程

在TCB中添加相应的记录（事实上也没有比在TCB上添加记录更好的选择，首先是TCB集成了该任务相关的信息，其次是TCB原本就有task_status等记录都内部可变)

```rust
pub struct TaskControlBlock {
  /// The task status in it's lifecycle
  pub task_status: TaskStatus,
  /// The task context
  pub task_cx: TaskContext,
  /// 记录每一个任务的开始时间(第一次被调度进CPU的时间,后续再调度也不会再被修改)
  pub start_time : usize,
  /// 记录每一个系统调用的使用次数(index是系统调用号,value是系统调用的次数)
  pub syscall_times:[u32;MAX_SYSCALL_NUM],
}
```

添加记录之后编译器会报错没有初始化变量 找到对应位置加入相关信息

```rust
let mut tasks = [TaskControlBlock {
    task_cx: TaskContext::zero_init(),
    task_status: TaskStatus::UnInit,
    syscall_times:[0;MAX_SYSCALL_NUM],
    start_time:0, //这里记为0是个标志位 第一次调度的时候发现是0 随后用当前时间替换掉 如果不为0就不再变化(代表当前时间)
}; MAX_APP_NUM];
```

修改完TCB的相关信息之后需要考虑一个点，我们如何给TCB提供这些信息？

##### 时间信息

首先是该任务开始运行的时间……

```rust
fn run_next_task(&self) {
    if let Some(next) = self.find_next_task() {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[next].task_status = TaskStatus::Running;
        if inner.tasks[next].start_time == 0{
            inner.tasks[next].start_time = get_time_us();//当调度下一个任务时，为其提供时间
        }
        ......
}
    
fn run_first_task(&self) -> ! {
    let mut inner = self.inner.exclusive_access();
    let task0 = &mut inner.tasks[0];
    task0.task_status = TaskStatus::Running;
    if task0.start_time == 0{
        task0.start_time == get_time_us();
    }
    ......//所有任务中第一个任务的开始时间也需要被记录
}
```

随后对外提供接口用于获取当前任务的时间和所处于的状态

```rust
impl TaskManager{
    fn get_current_taskcontrolblock_start_time(&self) -> usize{
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].start_time
    }

    fn get_current_taskcontrolblock_status(&self) -> usize{
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status
    }
}

///获得当前任务的起始时间
pub fn get_current_start_time() -> usize{
    TASK_MANAGER.get_current_taskcontrolblock_start_time()
}

///获得当前任务的状态
pub fn get_current_taskcontrolblock_status() -> TaskStatus{
    TASK_MANAGER.get_current_taskcontrolblock_status()
}
```

##### 系统调用次数信息

和时间信息是类似的，需要提供改变其的接口

```rust
impl TaskManager{
	fn add_syscall_times(&self,syscall_id:usize){
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_times[syscall_id] += 1;
    }

    fn get_syscall_times(&self) -> [u32;500]{
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_times
    }
}

//发生特定的系统调用 为其增加一次计数
pub fn add_syscall_times(syscall_id:usize){
    TASK_MANAGER.add_syscall_times(syscall_id);
}

//获取特定的系统调用次数
pub fn get_syscall_times(syscall_id:usize) -> [u32;500]{
    TASK_MANAGER.get_syscall_times()
}
```

但是和时间又有所不同，时间信息记录下来之后就不会再有变动 而`syscall`的次数每调用一次都需要增加

```rust
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    add_syscall_times(syscall_id); //发生系统调用 添加一次次数
    match syscall_id {
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(args[0] as *mut TimeVal, args[1]),
        SYSCALL_TASK_INFO => sys_task_info(args[0] as *mut TaskInfo),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
```

##### TaskInfo实现

```rust
/// YOUR JOB: Finish sys_task_info to pass testcases
//ti变量原本未使用，标记为_ti 此处记得修改
pub fn sys_task_info(ti: *mut TaskInfo) -> isize { 
    trace!("kernel: sys_task_info");
    unsafe{
        *ti = TaskInfo{
            status:get_current_taskcontrolblock_status(),
            syscall_times:get_syscall_times(),
            time: (get_time_us() - get_current_start_time())/1000
        };
    }
    0 //如果没什么问题返回0
}
```

#### 一些记录

报错挺多的，反正就照着编译器一个一个来吧……

类型错误……

<img src="D:\116\sigs\way2sigs-main\912\一轮复习\rcore_lab.assets\image-20240428141340827.png" alt="image-20240428141340827" style="zoom:67%;" />

missing documentations for functions

不写注释也不行 ~~笑~~

<img src="D:\116\sigs\way2sigs-main\912\一轮复习\rcore_lab.assets\image-20240428142025031.png" alt="image-20240428142025031"  />


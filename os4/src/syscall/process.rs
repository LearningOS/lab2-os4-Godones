//! Process management syscalls

use crate::config::MAX_SYSCALL_NUM;
use crate::mm::{MapPermission, PageTable, translated_refmut, VirtAddr};
use crate::task::{current_add_area, current_delete_page, current_user_token, exit_current_and_run_next, get_current_task_first_run_time, get_current_task_syscall, suspend_current_and_run_next, TaskStatus};
use crate::timer::get_time_us;


#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

// YOUR JOB: 引入虚地址后重写 sys_get_time
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let us = get_time_us();
    // 先找到当前任务的token
    // 通过构造pagetable来进行转换得到物理地址
    // 再通过物理地址获设置对应时间
    let token = current_user_token();
    let ptr = translated_refmut(token,ts);
    // debug!("[kernel] us: {},ptr: {:#x}",us,ptr as *mut TimeVal as usize);
    *ptr = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    0
}

// CLUE: 从 ch4 开始不再对调度算法进行测试~
pub fn sys_set_priority(_prio: isize) -> isize {
    -1
}

// YOUR JOB: 引入虚地址后重写 sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let token = current_user_token();
    let ptr = translated_refmut(token,ti);
    let time = get_time_us()/1000 - get_current_task_first_run_time();
    let syscall_times = get_current_task_syscall();
    *ptr = TaskInfo {
        status: TaskStatus::Running,
        syscall_times,
        time
    };
    0
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap

pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    let start_vir: VirtAddr = start.into(); //与页大小对齐
    //除了低8位其它位必须为0;
    //低8位不能全部为0
    if start_vir.aligned() != true || (port & !0x7 != 0) || (port & 0x7 == 0) {
        return -1;
    }
    //判断是否已经存在某个页被映射
    let new_port: u8 = (port & 0x7) as u8;
    let permission = MapPermission::U;
    let map_permission = MapPermission::from_bits(new_port << 1).unwrap() | permission;

    let start_vpn = start_vir.floor(); //起始页
    let end_vpn = VirtAddr::from(start + len).ceil(); //向上取整结束页

    error!("[kernel] start_vpn: {:#x}, end_vpn: {:#x}", start_vpn.0, end_vpn.0);
    //申请到一个map_area后判断其每个页是否出现在map_area中过
    let current_user_token = current_user_token(); //获取当前用户程序的satp
    let temp_page_table = PageTable::from_token(current_user_token);
    for vpn in start_vpn.0 ..end_vpn.0 {
        if let Some(_val) = temp_page_table.find_pte(vpn.into()) {
            error!("[kernel] mmap failed, page {:#x} already exists",vpn);
            return -1;
        } //提前返回错误值
    }
    current_add_area(start_vir, (start + len).into(), map_permission);
    0
}
/// 撤销申请的空间
pub fn sys_munmap(start: usize, len: usize) -> isize {
    let start_vir: VirtAddr = start.into(); //与页大小对齐
    if !start_vir.aligned() {
        return -1;
    }
    let start_vpn = start_vir.floor(); //起始页
    let end_vpn = VirtAddr::from(start + len).ceil(); //向上取整结束页
    let current_user_token = current_user_token(); //获取当前用户程序的satp
    let temp_page_table = PageTable::from_token(current_user_token);
    for vpn in start_vpn.0..end_vpn.0 {
        if temp_page_table.find_pte(vpn.into()).is_none() {
            return -1;
        } //提前返回错误值,如果这些页存在不位于内存的则错误返回
    }
    current_delete_page(start_vir);
    0
}

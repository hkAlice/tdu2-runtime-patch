use core::ffi::c_void;

use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

use crate::runtime_log::{log_error, log_info, log_warn};

unsafe fn change_page_protection(addr: usize, len: usize, new_protect: u32) -> Option<u32> {
    let mut old_protect: u32 = 0;
    if VirtualProtect(addr as *mut c_void, len, new_protect, &mut old_protect) == 0 {
        log_error(
            "memory",
            &format!(
                "VirtualProtect set failed at {addr:#x}, len={len}, protect={new_protect:#x}"
            ),
        );
        return None;
    }
    Some(old_protect)
}

unsafe fn restore_page_protection(addr: usize, len: usize, old_protect: u32) {
    let mut ignored: u32 = 0;
    if VirtualProtect(addr as *mut c_void, len, old_protect, &mut ignored) == 0 {
        log_error(
            "memory",
            &format!(
                "VirtualProtect restore failed at {addr:#x}, len={len}, protect={old_protect:#x}"
            ),
        );
    }
}

pub(crate) unsafe fn patch_bytes(addr: usize, bytes: &[u8]) {
    if let Some(old_protect) = change_page_protection(addr, bytes.len(), PAGE_EXECUTE_READWRITE) {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
        restore_page_protection(addr, bytes.len(), old_protect);
        log_info("patch", &format!("Patched {} bytes at {addr:#x}", bytes.len()));
    }
}

pub(crate) unsafe fn patch_nop(addr: usize, len: usize) {
    if let Some(old_protect) = change_page_protection(addr, len, PAGE_EXECUTE_READWRITE) {
        std::ptr::write_bytes(addr as *mut u8, 0x90, len);
        restore_page_protection(addr, len, old_protect);
        log_info("patch", &format!("NOPed {len} bytes at {addr:#x}"));
    }
}

pub(crate) fn relative_jump_displacement(src: usize, dst: usize, instruction_len: usize) -> Option<i32> {
    let delta = dst as isize - (src as isize + instruction_len as isize);
    if delta < i32::MIN as isize || delta > i32::MAX as isize {
        None
    } else {
        Some(delta as i32)
    }
}

pub(crate) unsafe fn flush_region(addr: usize, len: usize, tag: &str) {
    if FlushInstructionCache(GetCurrentProcess(), addr as *const c_void, len) == 0 {
        log_warn(
            "patch",
            &format!("FlushInstructionCache failed for {tag} at {addr:#x}, len={len}"),
        );
    }
}

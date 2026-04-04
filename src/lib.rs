// src/lib.rs
#![cfg(target_arch = "x86")] // build i686-pc-windows-msvc

use core::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HINSTANCE, TRUE};
use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::LibraryLoader::{DisableThreadLibraryCalls, GetModuleHandleA};
use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateThread(
        lpthreadattributes: *const c_void,
        dwstacksize: usize,
        lpstartaddress: Option<unsafe extern "system" fn(*mut c_void) -> u32>,
        lpparameter: *mut c_void,
        dwcreationflags: u32,
        lpthreadid: *mut u32,
    ) -> *mut c_void;
}

fn log_line(s: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("tdu2_camera_hook.log")
    {
        let _ = writeln!(f, "{s}");
    }
}

unsafe fn change_page_protection(addr: usize, len: usize, new_protect: u32) -> Option<u32> {
    let mut old_protect: u32 = 0;
    if VirtualProtect(addr as *mut c_void, len, new_protect, &mut old_protect) == 0 {
        log_line(&format!(
            "VirtualProtect set failed at {addr:#x}, len={len}, protect={new_protect:#x}"
        ));
        return None;
    }
    Some(old_protect)
}

unsafe fn restore_page_protection(addr: usize, len: usize, old_protect: u32) {
    let mut ignored: u32 = 0;
    if VirtualProtect(addr as *mut c_void, len, old_protect, &mut ignored) == 0 {
        log_line(&format!(
            "VirtualProtect restore failed at {addr:#x}, len={len}, protect={old_protect:#x}"
        ));
    }
}

unsafe fn patch_bytes(addr: usize, bytes: &[u8]) {
    if let Some(old_protect) = change_page_protection(addr, bytes.len(), PAGE_EXECUTE_READWRITE) {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
        restore_page_protection(addr, bytes.len(), old_protect);
        log_line(&format!("Patched {} bytes at {addr:#x}", bytes.len()));
    }
}

unsafe fn patch_nop(addr: usize, len: usize) {
    if let Some(old_protect) = change_page_protection(addr, len, PAGE_EXECUTE_READWRITE) {
        std::ptr::write_bytes(addr as *mut u8, 0x90, len);
        restore_page_protection(addr, len, old_protect);
        log_line(&format!("NOPed {len} bytes at {addr:#x}"));
    }
}

unsafe fn flush_region(addr: usize, len: usize, tag: &str) {
    if FlushInstructionCache(GetCurrentProcess(), addr as *const c_void, len) == 0 {
        log_line(&format!(
            "FlushInstructionCache failed for {tag} at {addr:#x}, len={len}"
        ));
    }
}

unsafe extern "system" fn init_thread(_: *mut c_void) -> u32 {
    log_line("init_thread started");
    std::thread::sleep(std::time::Duration::from_secs(5));

    let module = GetModuleHandleA(b"TestDrive2.exe\0".as_ptr());
    if module.is_null() {
        log_line("GetModuleHandleA(TestDrive2.exe) failed");
        return 0;
    }

    let base = module as usize;
    log_line(&format!("base = {base:#x}"));

    // Phase accumulator patches in camera update function
    patch_bytes(base + 0x7BCFBE, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]);
    patch_bytes(base + 0x7BD001, &[0xD9, 0xD8, 0x90, 0x90, 0x90, 0x90]);
    patch_bytes(base + 0x7BD015, &[0xD9, 0xD8, 0x90, 0x90, 0x90, 0x90]);

    // Downstream write sites
    patch_nop(base + 0x7BDC44, 8);
    patch_nop(base + 0x7BDC4C, 8);

    // Shake LUT skip checks
    patch_bytes(base + 0x851244, &[0xEB]);
    patch_bytes(base + 0x851274, &[0xEB]);

    // Zero out FUN_00CA2130 amplitudes
    // Is this still needed?
    patch_bytes(base + 0x8A2281, &[0x0F,0x57,0xC0,0x90,0x90,0x90,0x90,0x90]); // movss xmm0,[...] -> xorps xmm0,xmm0
    patch_bytes(base + 0x8A229C, &[0x0F,0x57,0xC0,0x90,0x90,0x90,0x90,0x90]);

    // Flush patched regions to ensure CPU sees updated instructions
    flush_region(base + 0x7B0000, 0x20000, "camera update region");
    flush_region(base + 0x850000, 0x10000, "shake LUT region");

    log_line("all patches applied");
    0
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(hinst: HINSTANCE, reason: u32, _: *mut c_void) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            DisableThreadLibraryCalls(hinst);
            let thread_handle =
                CreateThread(null(), 0, Some(init_thread), null_mut(), 0, null_mut());
            if thread_handle.is_null() {
                log_line("CreateThread(init_thread) failed");
            } else {
                CloseHandle(thread_handle);
            }
        }
        DLL_PROCESS_DETACH => {}
        _ => {}
    }
    TRUE
}

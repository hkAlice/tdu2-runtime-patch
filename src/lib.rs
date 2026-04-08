#![cfg(target_arch = "x86")] // build i686-pc-windows-msvc

mod config;
mod features;
mod patch_utils;
mod proxy;
mod runtime_log;

use core::ffi::c_void;
use std::ptr::{null, null_mut};
use std::time::Duration;

use config::load_patch_config;
use features::anti_tamper::apply_anti_tamper_patches;
use features::camera::apply_camera_fix_patches;
use features::fov::apply_fov_multiplier_hook;
use runtime_log::{log_error, log_info, log_line, log_runtime_banner, log_warn};
use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HINSTANCE, TRUE};
use windows_sys::Win32::System::LibraryLoader::{DisableThreadLibraryCalls, GetModuleHandleA};
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

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

unsafe extern "system" fn init_thread(_: *mut c_void) -> u32 {
    log_runtime_banner();
    let config = load_patch_config();

    log_line("init_thread started");
    std::thread::sleep(Duration::from_secs(config.startup_delay_seconds));

    let module = GetModuleHandleA(b"TestDrive2.exe\0".as_ptr());
    if module.is_null() {
        log_error("runtime", "GetModuleHandleA(TestDrive2.exe) failed");
        return 0;
    }

    let base = module as usize;
    log_line(&format!("base = {base:#x}"));

    let mut enabled_groups = 0;

    if config.fov_enabled {
        if apply_fov_multiplier_hook(base, config.fov_multiplier) {
            enabled_groups += 1;
        } else {
            log_warn("fov", "FOV multiplier hook was not applied");
        }
    } else {
        log_info("fov", "FOV.Enabled=0, skipping FOV multiplier hook");
    }

    if config.anti_tamper_enabled {
        apply_anti_tamper_patches(base);
        enabled_groups += 1;
    } else {
        log_info(
            "anti_tamper",
            "AntiTamperEnabled=0, skipping anti-tamper patch group",
        );
    }

    if config.camera_fix_enabled {
        apply_camera_fix_patches(base);
        enabled_groups += 1;
    } else {
        log_info("camera", "CameraFixEnabled=0, skipping camera-fix patch group");
    }

    if enabled_groups == 0 {
        log_line("No patch groups enabled in config");
    } else {
        log_line(&format!("Applied {enabled_groups} patch group(s)"));
    }

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
                log_error("runtime", "CreateThread(init_thread) failed");
            } else {
                CloseHandle(thread_handle);
            }
        }
        DLL_PROCESS_DETACH => {}
        _ => {}
    }
    TRUE
}

// src/lib.rs
#![cfg(target_arch = "x86")] // build i686-pc-windows-msvc

use core::ffi::c_void;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::ptr::{null, null_mut};
use std::sync::Once;
use std::time::Duration;

use log::LevelFilter;
use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HINSTANCE, TRUE};
use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::LibraryLoader::{DisableThreadLibraryCalls, GetModuleHandleA};
use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

const CONFIG_FILE_NAME: &str = "tdu2-runtime-patch.ini";
const LOG_FILE_NAME: &str = "tdu2-runtime-patch.log";
const DEFAULT_STARTUP_DELAY_SECONDS: u64 = 3;
const PROJECT_NAME: &str = env!("CARGO_PKG_NAME");
const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");
static LOGGER_INIT: Once = Once::new();

#[derive(Clone, Copy)]
struct PatchConfig {
    anti_tamper_enabled: bool,
    camera_fix_enabled: bool,
    startup_delay_seconds: u64,
}

impl Default for PatchConfig {
    fn default() -> Self {
        Self {
            anti_tamper_enabled: true,
            camera_fix_enabled: true,
            startup_delay_seconds: DEFAULT_STARTUP_DELAY_SECONDS,
        }
    }
}

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

fn init_logger() -> Result<(), String> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE_NAME)
        .map_err(|err| format!("Failed to open log file {LOG_FILE_NAME}: {err}"))?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                time,
                record.level(),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Debug)
        .chain(file)
        .apply()
        .map_err(|err| format!("Failed to initialize logger: {err}"))
}

fn init_logger_once() {
    LOGGER_INIT.call_once(|| {
        if let Err(err) = init_logger() {
            let _ = writeln!(std::io::stderr(), "[{PROJECT_NAME}] {err}");
        }
    });
}

fn log_info(target: &'static str, message: &str) {
    init_logger_once();
    log::info!(target: target, "{}", message);
}

fn log_warn(target: &'static str, message: &str) {
    init_logger_once();
    log::warn!(target: target, "{}", message);
}

fn log_error(target: &'static str, message: &str) {
    init_logger_once();
    log::error!(target: target, "{}", message);
}

fn log_line(message: &str) {
    log_info("runtime", message);
}

fn log_runtime_banner() {
    let git_hash = option_env!("GIT_COMMIT_HASH").unwrap_or("unknown");
    log_info(
        "runtime",
        &format!(
        "{PROJECT_NAME} v{PROJECT_VERSION} (git {git_hash})"
        ),
    );
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn write_default_config_file() {
    let defaults = PatchConfig::default();
    let anti_tamper = if defaults.anti_tamper_enabled { 1 } else { 0 };
    let camera_fix = if defaults.camera_fix_enabled { 1 } else { 0 };

    let template = format!(
        "[Patch]\nAntiTamperEnabled = {anti_tamper}\nCameraFixEnabled = {camera_fix}\nStartupDelaySeconds = {}\n",
        defaults.startup_delay_seconds
    );

    match fs::write(CONFIG_FILE_NAME, template) {
        Ok(_) => log_info("config", &format!(
            "Created default config file: {CONFIG_FILE_NAME}"
        )),
        Err(err) => log_error("config", &format!(
            "Failed to create default config file {CONFIG_FILE_NAME}: {err}"
        )),
    }
}

fn load_patch_config() -> PatchConfig {
    let mut config = PatchConfig::default();

    let content = match fs::read_to_string(CONFIG_FILE_NAME) {
        Ok(content) => content,
        Err(err) => {
            log_warn("config", &format!(
                "Config file read failed ({CONFIG_FILE_NAME}): {err}. Using defaults."
            ));
            if err.kind() == std::io::ErrorKind::NotFound {
                write_default_config_file();
            }
            return config;
        }
    };

    let mut section = String::new();

    for (line_idx, raw_line) in content.lines().enumerate() {
        let line_without_semicolon_comment = raw_line.split(';').next().unwrap_or(raw_line);
        let line = line_without_semicolon_comment
            .split('#')
            .next()
            .unwrap_or(line_without_semicolon_comment)
            .trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };

        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value.trim();

        let full_key = if section.is_empty() {
            key.clone()
        } else {
            format!("{section}.{key}")
        };

        match full_key.as_str() {
            "patch.antitamperenabled" | "antitamperenabled" => {
                if let Some(parsed) = parse_bool(value) {
                    config.anti_tamper_enabled = parsed;
                } else {
                    log_warn("config", &format!(
                        "Invalid bool for AntiTamperEnabled on line {}: {value}",
                        line_idx + 1
                    ));
                }
            }
            "patch.camerafixenabled" | "camerafixenabled" => {
                if let Some(parsed) = parse_bool(value) {
                    config.camera_fix_enabled = parsed;
                } else {
                    log_warn("config", &format!(
                        "Invalid bool for CameraFixEnabled on line {}: {value}",
                        line_idx + 1
                    ));
                }
            }
            "patch.startupdelayseconds" | "startupdelayseconds" => {
                if let Ok(parsed) = value.parse::<u64>() {
                    config.startup_delay_seconds = parsed;
                } else {
                    log_warn("config", &format!(
                        "Invalid integer for StartupDelaySeconds on line {}: {value}",
                        line_idx + 1
                    ));
                }
            }
            _ => {}
        }
    }

    log_info("config", &format!(
        "Config loaded: AntiTamperEnabled={}, CameraFixEnabled={}, StartupDelaySeconds={}",
        config.anti_tamper_enabled, config.camera_fix_enabled, config.startup_delay_seconds
    ));

    config
}

unsafe fn change_page_protection(addr: usize, len: usize, new_protect: u32) -> Option<u32> {
    let mut old_protect: u32 = 0;
    if VirtualProtect(addr as *mut c_void, len, new_protect, &mut old_protect) == 0 {
        log_error("memory", &format!(
            "VirtualProtect set failed at {addr:#x}, len={len}, protect={new_protect:#x}"
        ));
        return None;
    }
    Some(old_protect)
}

unsafe fn restore_page_protection(addr: usize, len: usize, old_protect: u32) {
    let mut ignored: u32 = 0;
    if VirtualProtect(addr as *mut c_void, len, old_protect, &mut ignored) == 0 {
        log_error("memory", &format!(
            "VirtualProtect restore failed at {addr:#x}, len={len}, protect={old_protect:#x}"
        ));
    }
}

unsafe fn patch_bytes(addr: usize, bytes: &[u8]) {
    if let Some(old_protect) = change_page_protection(addr, bytes.len(), PAGE_EXECUTE_READWRITE) {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
        restore_page_protection(addr, bytes.len(), old_protect);
        log_info("patch", &format!("Patched {} bytes at {addr:#x}", bytes.len()));
    }
}

unsafe fn patch_nop(addr: usize, len: usize) {
    if let Some(old_protect) = change_page_protection(addr, len, PAGE_EXECUTE_READWRITE) {
        std::ptr::write_bytes(addr as *mut u8, 0x90, len);
        restore_page_protection(addr, len, old_protect);
        log_info("patch", &format!("NOPed {len} bytes at {addr:#x}"));
    }
}

unsafe fn flush_region(addr: usize, len: usize, tag: &str) {
    if FlushInstructionCache(GetCurrentProcess(), addr as *const c_void, len) == 0 {
        log_warn("patch", &format!(
            "FlushInstructionCache failed for {tag} at {addr:#x}, len={len}"
        ));
    }
}

unsafe fn apply_anti_tamper_patches(base: usize) {
    log_info("anti_tamper", "Applying anti-tamper patch group");

    // NOP the trigger -> MOV [ECX+0x128], 0x25
    // 00D63F56: C7 81 28 01 00 00 25 00 00 00  (10 bytes)
    patch_nop(base + 0x9C3F56, 10);
    flush_region(base + 0x9C0000, 0x10000, "anti tamper region");

    // Zero the IsDebuggerPresent func ptr so it never gets called
    // DAT_01459EA4 - 0x400000 = 0x1059EA4
    patch_bytes(base + 0x1059EA4, &[0x00, 0x00, 0x00, 0x00]);
    flush_region(base + 0x1050000, 0x10000, "anti debug region");

    // Killswitch writes (warning: used by normal game shutdown too):
    // 00D52557: C6 85 31 01 00 00 01
    // 00D5411F: C6 82 31 01 00 00 01
    // 00D542AF: C6 81 31 01 00 00 01
    // 00D542EF: C6 80 31 01 00 00 01
    // 00D54457: C6 82 31 01 00 00 01
    /*
    log_info(
        "anti_tamper",
        &format!("neutralizing kill switch at {:#x}", 0x9525EC),
    );
    patch_bytes(base + 0x95255D, &[0x00]);
    patch_bytes(base + 0x954125, &[0x00]);
    patch_bytes(base + 0x9542B5, &[0x00]);
    patch_bytes(base + 0x9542F5, &[0x00]);
    patch_bytes(base + 0x95445D, &[0x00]);
    */

    // 00D525EC: CALL FUN_00D97F90
    patch_nop(base + 0x9525EC, 5);

    // FUN_008BB8F0 writes this->0x114 = 4
    // VA 008BB9B9 : C7 86 14 01 00 00 04 00 00 00
    // Change 4 -> 1 (keeps nonzero error semantics, avoids == 4 killswitch)
    log_info(
        "anti_tamper",
        &format!("neutralizing flag switch at {:#x}", 0x49C91D),
    );
    patch_bytes(base + 0x4BB9BF, &[0x01]);

    flush_region(base + 0x4B0000, 0x10000, "first flag quit region");
    flush_region(base + 0x490000, 0x10000, "second flag quit region");
    flush_region(base + 0x950000, 0x10000, "killswitch region");
}

unsafe fn apply_camera_fix_patches(base: usize) {
    log_info("camera", "Applying camera-fix patch group");

    let before = std::slice::from_raw_parts((base + 0x7BD001) as *const u8, 8);
    log_info("camera", &format!("0x7BD001 before patch: {:02x?}", before));

    let before2 = std::slice::from_raw_parts((base + 0x7BD015) as *const u8, 8);
    log_info("camera", &format!("0x7BD015 before patch: {:02x?}", before2));

    // Phase accumulator patches in camera update function
    patch_bytes(base + 0x7BCFBE, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]);
    patch_bytes(base + 0x7BD001, &[0xD9, 0xD8, 0x90, 0x90, 0x90, 0x90]);
    patch_bytes(base + 0x7BD015, &[0xD9, 0xD8, 0x90, 0x90, 0x90, 0x90]);

    let after = std::slice::from_raw_parts((base + 0x7BD001) as *const u8, 8);
    log_info("camera", &format!("0x7BD001 after patch: {:02x?}", after));

    let after2 = std::slice::from_raw_parts((base + 0x7BD015) as *const u8, 8);
    log_info("camera", &format!("0x7BD015 after patch: {:02x?}", after2));

    // Downstream write sites
    patch_nop(base + 0x7BDC44, 8);
    patch_nop(base + 0x7BDC4C, 8);

    // Shake LUT skip checks
    patch_bytes(base + 0x851244, &[0xEB]);
    patch_bytes(base + 0x851274, &[0xEB]);

    // Zero out FUN_00CA2130 amplitudes
    // movss xmm0,[...] -> xorps xmm0,xmm0
    patch_bytes(
        base + 0x8A2281,
        &[0x0F, 0x57, 0xC0, 0x90, 0x90, 0x90, 0x90, 0x90],
    );
    patch_bytes(
        base + 0x8A229C,
        &[0x0F, 0x57, 0xC0, 0x90, 0x90, 0x90, 0x90, 0x90],
    );

    // FUN_00C80B00 @ 0x00C81AA0: Replace "FMUL [constant]" with
    // "FMUL [EBP+0xC]" (deltaTime) to avoid frame-dependent jitter.
    // Original: DC 0D 68 AB F4 00  (FMUL double ptr [0x00F4AB68])
    // Patched:  D8 4D 0C 90 90 90  (FMUL dword ptr [EBP+0xC], NOP, NOP, NOP)
    log_info("camera", "Patching camera frame-time compensation");
    patch_bytes(base + 0x8C1AA0, &[0xD8, 0x4D, 0x0C, 0x90, 0x90, 0x90]);

    // Patch bug: if (param_3 < fVar21 - fVar21)
    // 00BBD440: FLD ST0 (D9 C0) -> FLDZ (D9 EE)
    patch_bytes(base + 0x7BD440, &[0xD9, 0xEE]);

    // FSTP [EDI-0x134] -> FSTP ST0, discard suspension feed write
    patch_bytes(base + 0x7BD4A9, &[0xDC, 0x0D, 0x88, 0x9E, 0xF4, 0x00]);
    patch_bytes(base + 0x7BD4AF, &[0xD9, 0xD8, 0x90, 0x90, 0x90, 0x90]);

    flush_region(base + 0x7B0000, 0x20000, "camera update region");
    flush_region(base + 0x850000, 0x10000, "shake LUT region");
    flush_region(base + 0x8A0000, 0x10000, "amplitude region");
    flush_region(base + 0x8C0000, 0x10000, "camera position region");
}

unsafe extern "system" fn init_thread(_: *mut c_void) -> u32 {
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

    if config.anti_tamper_enabled {
        apply_anti_tamper_patches(base);
        enabled_groups += 1;
    } else {
        log_info("anti_tamper", "AntiTamperEnabled=0, skipping anti-tamper patch group");
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
            log_runtime_banner();
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

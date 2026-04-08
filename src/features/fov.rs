use std::ptr::null_mut;

use windows_sys::Win32::System::Memory::{
    VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};

use crate::patch_utils::{flush_region, patch_bytes, relative_jump_displacement};
use crate::runtime_log::{log_error, log_info};

const DEFAULT_FOV_MULTIPLIER: f32 = 1.2;
const FOV_HOOK_OFFSET: usize = 0x89260F;
const FOV_RETURN_OFFSET: usize = 0x892615;
static mut FOV_MULTIPLIER_VALUE: f32 = DEFAULT_FOV_MULTIPLIER;

pub(crate) unsafe fn apply_fov_multiplier_hook(base: usize, multiplier: f32) -> bool {
    let hook_addr = base + FOV_HOOK_OFFSET;
    let return_addr = base + FOV_RETURN_OFFSET;
    let expected_original = [0xD9, 0x5C, 0x24, 0x10, 0xFF, 0xD2];
    let current = std::slice::from_raw_parts(hook_addr as *const u8, expected_original.len());

    if current != expected_original {
        log_error(
            "fov",
            &format!(
                "Unexpected bytes at hook site {hook_addr:#x}: got {:02x?}, expected {:02x?}",
                current, expected_original
            ),
        );
        return false;
    }

    FOV_MULTIPLIER_VALUE = multiplier;

    let cave = VirtualAlloc(
        null_mut(),
        0x1000,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    ) as usize;

    if cave == 0 {
        log_error("fov", "VirtualAlloc failed for FOV code cave");
        return false;
    }

    let multiplier_addr = core::ptr::addr_of!(FOV_MULTIPLIER_VALUE) as usize;
    let cave_jmp_addr = cave + 12;

    let Some(return_rel) = relative_jump_displacement(cave_jmp_addr, return_addr, 5) else {
        log_error(
            "fov",
            &format!(
                "Return jump out of range: cave_jmp={cave_jmp_addr:#x}, return={return_addr:#x}"
            ),
        );
        return false;
    };

    let mut cave_code = [
        0xD8, 0x0D, 0, 0, 0, 0, // fmul dword ptr [mult]
        0xD9, 0x5C, 0x24, 0x10, // fstp dword ptr [esp+10]
        0xFF, 0xD2, // call edx
        0xE9, 0, 0, 0, 0, // jmp return
    ];
    cave_code[2..6].copy_from_slice(&(multiplier_addr as u32).to_le_bytes());
    cave_code[13..17].copy_from_slice(&return_rel.to_le_bytes());

    std::ptr::copy_nonoverlapping(cave_code.as_ptr(), cave as *mut u8, cave_code.len());
    flush_region(cave, cave_code.len(), "fov cave");

    let Some(hook_rel) = relative_jump_displacement(hook_addr, cave, 5) else {
        log_error(
            "fov",
            &format!("Hook jump out of range: hook={hook_addr:#x}, cave={cave:#x}"),
        );
        return false;
    };

    let mut hook_patch = [0xE9, 0, 0, 0, 0, 0x90];
    hook_patch[1..5].copy_from_slice(&hook_rel.to_le_bytes());

    patch_bytes(hook_addr, &hook_patch);
    flush_region(hook_addr, hook_patch.len(), "fov hook");

    log_info(
        "fov",
        &format!(
            "FOV hook applied: hook={hook_addr:#x}, cave={cave:#x}, return={return_addr:#x}, multiplier={:.3}",
            multiplier
        ),
    );

    true
}

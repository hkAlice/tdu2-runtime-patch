use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use windows_sys::Win32::System::Memory::{
    VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};

use crate::patch_utils::{flush_region, patch_bytes, relative_jump_displacement};
use crate::runtime_log::{log_error, log_info};

const IS_DLC_OFFSET: usize = 0x82AF20;
const IS_DLC_HOOK_LEN: usize = 5;

type IsDlcFn = unsafe extern "stdcall" fn(*mut u8, u32, u32, u32) -> *mut u8;

static DLC_BASE: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_DLC_TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);
static DLC_PROBE_INSTALLED: AtomicBool = AtomicBool::new(false);

#[inline]
unsafe fn read_u8(base: usize, rva: usize) -> u8 {
    *((base + rva) as *const u8)
}

#[inline]
unsafe fn read_u32(base: usize, rva: usize) -> u32 {
    core::ptr::read_unaligned((base + rva) as *const u32)
}

#[inline]
unsafe fn read_ret_u8(ret: *const u8, offset: usize) -> u8 {
    *ret.add(offset)
}

#[inline]
unsafe fn read_ret_u32(ret: *const u8, offset: usize) -> u32 {
    core::ptr::read_unaligned(ret.add(offset) as *const u32)
}

unsafe extern "stdcall" fn probe_is_dlc(out: *mut u8, dlc_type: u32, lo: u32, hi: u32) -> *mut u8 {
    let orig_ptr = ORIG_IS_DLC_TRAMPOLINE.load(Ordering::Relaxed);
    if orig_ptr == 0 {
        return out;
    }

    let orig_fn: IsDlcFn = core::mem::transmute(orig_ptr);
    let ret = orig_fn(out, dlc_type, lo, hi);

    if ret.is_null() {
        return ret;
    }

    let base = DLC_BASE.load(Ordering::Relaxed);
    if base == 0 {
        return ret;
    }

    let status = read_ret_u32(ret as *const u8, 0x10);
    let phys_present = read_ret_u8(ret as *const u8, 0x14);
    let purchased = read_ret_u8(ret as *const u8, 0x15);
    let packed = read_ret_u8(ret as *const u8, 0x1C);

    let e9 = read_u8(base, 0x0C017E9);
    let c0 = read_u8(base, 0x0E545C0);
    let c1 = read_u8(base, 0x0E545C1);
    let c4 = read_u32(base, 0x0E545C4);
    let cc = read_u32(base, 0x0E545CC);
    let head = read_u32(base, 0x0E545D8);
    let tail = read_u32(base, 0x0E545DC);
    let cnt = read_u32(base, 0x0E545E0);

    log_info(
        "dlc",
        &format!(
            "DLC t={} h={:08X}:{:08X} st={} phys={} buy={} pack={} | e9={} c0={} c1={} c4={} cc={} head={:08X} tail={:08X} cnt={}",
            dlc_type,
            hi,
            lo,
            status,
            phys_present,
            purchased,
            packed,
            e9,
            c0,
            c1,
            c4,
            cc,
            head,
            tail,
            cnt
        ),
    );

    ret
}

unsafe fn install_dlc_probe_hook(base: usize) -> bool {
    if DLC_PROBE_INSTALLED.load(Ordering::Relaxed) {
        return true;
    }

    let hook_addr = base + IS_DLC_OFFSET;
    let mut original = [0u8; IS_DLC_HOOK_LEN];
    std::ptr::copy_nonoverlapping(
        hook_addr as *const u8,
        original.as_mut_ptr(),
        IS_DLC_HOOK_LEN,
    );

    if original[0] == 0xE9 {
        log_info("dlc", "IsDlc probe hook skipped: hook appears to already be present");
        DLC_PROBE_INSTALLED.store(true, Ordering::Relaxed);
        return true;
    }

    let trampoline = VirtualAlloc(
        null_mut(),
        0x1000,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    ) as usize;

    if trampoline == 0 {
        log_error("dlc", "VirtualAlloc failed for IsDlc trampoline");
        return false;
    }

    std::ptr::copy_nonoverlapping(
        original.as_ptr(),
        trampoline as *mut u8,
        IS_DLC_HOOK_LEN,
    );

    let trampoline_jmp_addr = trampoline + IS_DLC_HOOK_LEN;
    let Some(back_rel) =
        relative_jump_displacement(trampoline_jmp_addr, hook_addr + IS_DLC_HOOK_LEN, 5)
    else {
        log_error(
            "dlc",
            &format!(
                "Trampoline jump out of range: tramp_jmp={trampoline_jmp_addr:#x}, return={:#x}",
                hook_addr + IS_DLC_HOOK_LEN
            ),
        );
        return false;
    };

    let mut back_jmp = [0xE9, 0, 0, 0, 0];
    back_jmp[1..5].copy_from_slice(&back_rel.to_le_bytes());
    std::ptr::copy_nonoverlapping(back_jmp.as_ptr(), trampoline_jmp_addr as *mut u8, back_jmp.len());
    flush_region(
        trampoline,
        IS_DLC_HOOK_LEN + back_jmp.len(),
        "DLC probe trampoline",
    );

    let probe_addr = probe_is_dlc as *const () as usize;
    let Some(hook_rel) = relative_jump_displacement(hook_addr, probe_addr, 5) else {
        log_error(
            "dlc",
            &format!(
                "IsDlc hook jump out of range: hook={hook_addr:#x}, probe={probe_addr:#x}"
            ),
        );
        return false;
    };

    let mut hook_patch = [0xE9, 0, 0, 0, 0];
    hook_patch[1..5].copy_from_slice(&hook_rel.to_le_bytes());

    DLC_BASE.store(base, Ordering::Relaxed);
    ORIG_IS_DLC_TRAMPOLINE.store(trampoline, Ordering::Relaxed);

    patch_bytes(hook_addr, &hook_patch);
    flush_region(hook_addr, hook_patch.len(), "DLC probe hook");

    DLC_PROBE_INSTALLED.store(true, Ordering::Relaxed);

    log_info(
        "dlc",
        &format!(
            "Installed IsDlc probe hook: hook={hook_addr:#x}, trampoline={trampoline:#x}, stolen={:02X?}",
            original
        ),
    );

    true
}

pub(crate) unsafe fn apply_dlc_car_dealer_patches(base: usize) {
    log_info("dlc", "Applying DLC car dealer fix");

    //let _ = install_dlc_probe_hook(base);

    //patch_bytes(base + 0x82AF5A, &[0xEB]);

    // Set state = 2 (PURCHASED)
    //patch_bytes(base + 0x82AF77, &[0xC7, 0x45, 0x10, 0x02, 0x00, 0x00, 0x00]);
    // Set purchased = 1 (true), was AL (0)
    //patch_bytes(base + 0x82AF7E, &[0xC6, 0x45, 0x15, 0x01]);

    /*
    FUN_009F6890 writes _root.buy.vehiclelist.is_dlc_line, and for DLC-tagged cars ([car+0x221] != 0) it does: 
    is_dlc_line = (FUN_009A69A0(...) != 0).
    Offline, DAT_012545C1 == 0 makes FUN_009A69A0 force state 2 (at 009A6A68), so it returns nonzero and the line stays
    DLC/locked.

    UI patch (outside the SecuROM-sensitive 0xC2xxxx area):
    */

    // (009F696B: JZ -> JMP, always takes non-DLC path for list row classification)
    // UI "blue" effect
    patch_bytes(base + 0x5F696B, &[0xEB]);
    patch_bytes(base + 0x59DE9F, &[0xEB]);
    patch_bytes(base + 0x9A5296, &[0xEB]);

    /*
    patch_bytes(base + 0x6B8C0B, &[0xEB]);
    patch_bytes(base + 0x6B8D73, &[0xEB]);
    */

    // FUN_009EEAA
    // 009EEB58 checks if enough money to buy
    // ignore if DLC car? unsure on retail behavior

    flush_region(base + 0x5F0000, 0x10000, "DLC car dealer region");
}
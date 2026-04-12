use crate::patch_utils::{flush_region, patch_bytes, patch_nop};
use crate::runtime_log::log_info;

pub(crate) unsafe fn apply_camera_fix_patches(base: usize) {
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
    // "FMUL [EBP+0xC]" (deltaTime) to avoid frame-dependent jitter
    // Original: DC 0D 68 AB F4 00  (FMUL double ptr [0x00F4AB68])
    // Patched:  D8 4D 0C 90 90 90  (FMUL dword ptr [EBP+0xC], NOP, NOP, NOP)
    log_info("camera", "Patching camera frame-time compensation");
    patch_bytes(base + 0x881AA0, &[0xD8, 0x4D, 0x0C, 0x90, 0x90, 0x90]);

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

    // interesting notes:

    // FUN_00ca25a0 -> XMM0 near clip, ESP far clip/"draw distance" (not LOD)

    // Camera mode dependent speed @ 0x00c90714:
    /*
      iVar2 = *(int *)(unaff_EDI + 0x2e8);
        if (iVar2 == 0x16) {
            fVar14 = 4.0;
        }
        else if (iVar2 == 0x17) {
            fVar14 = 2.0;
        }
        else if (iVar2 == 0x18) {
            fVar14 = 6.0;
        }
        else {
            fVar14 = 1.5;
        }
     */
}

pub(crate) unsafe fn apply_camera_shake_patch(base: usize) {
    log_info("camera", "Patching exterior camera shake (1459F44/1459F48)");

    /*
    disables camera turning?? wtf
    // TEST ECX,0x1000 -> TEST ECX,0
    patch_bytes(base + 0x8907CC, &[0x00]);
    // TEST ECX,0x2000 -> TEST ECX,0
    patch_bytes(base + 0x89083F, &[0x00]);
    // TEST CL,0x20 -> TEST CL,0  (kills +0x385/386/387/64D accumulation)
    patch_bytes(base + 0x88846F, &[0x00]);
    // TEST CL,0x40 -> TEST CL,0  (kills +0x5EE/5EF path)
    patch_bytes(base + 0x888539, &[0x00]);

    flush_region(base + 0x880000, 0x10000, "exterior camera jitter region");
    flush_region(base + 0x890000, 0x10000, "exterior camera jitter region");
    */
    /*
    patch_nop(base + 0x86BBEF, 5);
    patch_nop(base + 0x86BBF4, 5);
    patch_nop(base + 0x86DA46, 5);
    patch_nop(base + 0x86DAD9, 5);
    patch_nop(base + 0x86DAF9, 5);

    // 00C90968: imm8 of TEST [EDI+300], imm8
    patch_bytes(base + 0x890968, &[0x00]);   // was 0x20*/


    // Exterior shake accumulators
    
    patch_bytes(base + 0x88F790, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]); // FLD [1459F48] -> FLDZ
    patch_bytes(base + 0x88F7BD, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]); // FLD [1459F48] -> FLDZ
    patch_bytes(base + 0x88F735, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]); // FLD [1459F44] -> FLDZ

    flush_region(base + 0x880000, 0x10000, "exterior camera jitter region");
}

/*
pub(crate) unsafe fn apply_camera_offroad_jitter_patch(base: usize) {
    log_info("camera", "Patching offroad camera shake (1459F4C)");

    // Offroad-specific shake source
    patch_bytes(base + 0x88F7BD, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]);
    patch_bytes(base + 0x88F708, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]); // FLD [1459F44] -> FLDZ
    patch_bytes(base + 0x88F735, &[0xD9, 0xEE, 0x90, 0x90, 0x90, 0x90]); // FLD [1459F44] -> FLDZ

    flush_region(base + 0x880000, 0x10000, "offroad camera jitter region");
}
*/
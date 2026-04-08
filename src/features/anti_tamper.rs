use crate::patch_utils::{flush_region, patch_bytes, patch_nop};
use crate::runtime_log::log_info;

pub(crate) unsafe fn apply_anti_tamper_patches(base: usize) {
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

    // 00D63F35: FF 15 E8 11 E4 00   ; CALL [IsDebuggerPresent IAT]
    // force IsDebuggerPresent return 0 for debuggers who don't patch it out
    // XOR EAX,EAX + NOPs
    patch_bytes(base + 0x963F35, &[0x31, 0xC0, 0x90, 0x90, 0x90, 0x90]);

    // 00D63F11: 0B C1          ; OR EAX, ECX
    // clear latched debug flag result before SETNZ
    patch_bytes(base + 0x963F11, &[0x31, 0xC0]); // XOR EAX,EAX

    flush_region(base + 0x4B0000, 0x10000, "first flag quit region");
    flush_region(base + 0x490000, 0x10000, "second flag quit region");
    flush_region(base + 0x950000, 0x10000, "killswitch region");
    flush_region(base + 0x960000, 0x10000, "debug check region");
}

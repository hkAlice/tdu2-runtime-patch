use crate::patch_utils::{flush_region, patch_bytes};
use crate::runtime_log::log_info;

pub(crate) unsafe fn apply_dlc_car_dealer_patches(base: usize) {
    log_info("dlc", "Applying DLC car dealer fix");

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
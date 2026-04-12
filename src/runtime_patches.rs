use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::config::{load_patch_config, save_patch_config, PatchConfig};
use crate::features::anti_tamper::apply_anti_tamper_patches;
use crate::features::camera::{apply_camera_fix_patches, apply_camera_shake_patch};
use crate::features::dlc::apply_dlc_car_dealer_patches;
use crate::features::fov::{apply_fov_multiplier_hook, set_fov_multiplier_value};
use crate::overlay::set_overlay_panel_feature_state;
use crate::patch_utils::{flush_region, patch_bytes};
use crate::runtime_log::{log_error, log_info, log_warn};

const ANTI_TAMPER_REGIONS: &[(usize, usize, &str)] = &[
    (0x490000, 0x10000, "anti_tamper restore region 0x490000"),
    (0x4B0000, 0x10000, "anti_tamper restore region 0x4B0000"),
    (0x950000, 0x10000, "anti_tamper restore region 0x950000"),
    (0x960000, 0x10000, "anti_tamper restore region 0x960000"),
    (0x9C0000, 0x10000, "anti_tamper restore region 0x9C0000"),
    (0x1050000, 0x10000, "anti_tamper restore region 0x1050000"),
];

const DLC_FIX_REGIONS: &[(usize, usize, &str)] = &[
    (0x590000, 0x10000, "dlc restore region 0x590000"),
    (0x5F0000, 0x10000, "dlc restore region 0x5F0000"),
    (0x9A0000, 0x10000, "dlc restore region 0x9A0000"),
];

const CAMERA_FIX_REGIONS: &[(usize, usize, &str)] = &[
    (0x7B0000, 0x20000, "camera restore region 0x7B0000"),
    (0x850000, 0x10000, "camera restore region 0x850000"),
    (0x8A0000, 0x10000, "camera restore region 0x8A0000"),
    (0x8C0000, 0x10000, "camera restore region 0x8C0000"),
];

const CAMERA_SHAKE_FIX_REGIONS: &[(usize, usize, &str)] =
    &[(0x880000, 0x10000, "camera shake restore region 0x880000")];

const FOV_REGIONS: &[(usize, usize, &str)] = &[(0x890000, 0x10000, "fov restore region 0x890000")];

#[derive(Clone)]
struct RegionSnapshot {
    address: usize,
    bytes: Vec<u8>,
    flush_tag: String,
}

struct ToggleState {
    enabled: bool,
    snapshots: Option<Vec<RegionSnapshot>>,
}

impl ToggleState {
    fn new() -> Self {
        Self {
            enabled: false,
            snapshots: None,
        }
    }
}

struct RuntimePatchController {
    base: usize,
    anti_tamper: ToggleState,
    dlc_fix: ToggleState,
    camera_fix: ToggleState,
    camera_shake_fix: ToggleState,
    fov: ToggleState,
    fov_multiplier: f32,
}

impl RuntimePatchController {
    fn new() -> Self {
        Self {
            base: 0,
            anti_tamper: ToggleState::new(),
            dlc_fix: ToggleState::new(),
            camera_fix: ToggleState::new(),
            camera_shake_fix: ToggleState::new(),
            fov: ToggleState::new(),
            fov_multiplier: 1.2,
        }
    }

    fn set_base(&mut self, base: usize) {
        if self.base == 0 {
            self.base = base;
            return;
        }

        if self.base != base {
            log_warn(
                "runtime",
                &format!(
                    "Runtime patch base address changed from {:#x} to {:#x}; keeping the first value",
                    self.base, base
                ),
            );
        }
    }

    fn ensure_base(&self, feature: &str) -> Option<usize> {
        if self.base == 0 {
            log_error(
                "runtime",
                &format!("{feature}: runtime base address is not initialized"),
            );
            None
        } else {
            Some(self.base)
        }
    }

    fn panel_state(&self) -> RuntimePatchPanelState {
        RuntimePatchPanelState {
            anti_tamper_enabled: self.anti_tamper.enabled,
            dlc_fix_enabled: self.dlc_fix.enabled,
            camera_fix_enabled: self.camera_fix.enabled,
            camera_shake_fix_enabled: self.camera_shake_fix.enabled,
            fov_enabled: self.fov.enabled,
            fov_multiplier: self.fov_multiplier,
        }
    }

    fn set_fov_multiplier(&mut self, multiplier: f32) -> f32 {
        let sanitized = sanitize_fov_multiplier(multiplier, self.fov_multiplier);
        self.fov_multiplier = sanitized;
        unsafe {
            set_fov_multiplier_value(sanitized);
        }
        sanitized
    }

    unsafe fn set_anti_tamper_enabled(&mut self, enabled: bool) -> bool {
        let Some(base) = self.ensure_base("AntiTamper") else {
            return self.anti_tamper.enabled;
        };

        if enabled {
            if self.anti_tamper.enabled {
                return true;
            }

            ensure_snapshots(base, &mut self.anti_tamper, ANTI_TAMPER_REGIONS);
            apply_anti_tamper_patches(base);
            self.anti_tamper.enabled = true;
            log_info("anti_tamper", "AntiTamper runtime state: ON");
            true
        } else {
            disable_feature_with_restore("AntiTamper", &mut self.anti_tamper)
        }
    }

    unsafe fn set_dlc_fix_enabled(&mut self, enabled: bool) -> bool {
        let Some(base) = self.ensure_base("DlcCarDealerFix") else {
            return self.dlc_fix.enabled;
        };

        if enabled {
            if self.dlc_fix.enabled {
                return true;
            }

            ensure_snapshots(base, &mut self.dlc_fix, DLC_FIX_REGIONS);
            apply_dlc_car_dealer_patches(base);
            self.dlc_fix.enabled = true;
            log_info("dlc", "DlcCarDealerFix runtime state: ON");
            true
        } else {
            disable_feature_with_restore("DlcCarDealerFix", &mut self.dlc_fix)
        }
    }

    unsafe fn set_camera_fix_enabled(&mut self, enabled: bool) -> bool {
        let Some(base) = self.ensure_base("CameraFix") else {
            return self.camera_fix.enabled;
        };

        if enabled {
            if self.camera_fix.enabled {
                return true;
            }

            ensure_snapshots(base, &mut self.camera_fix, CAMERA_FIX_REGIONS);
            apply_camera_fix_patches(base);
            self.camera_fix.enabled = true;
            log_info("camera", "CameraFix runtime state: ON");
            true
        } else {
            disable_feature_with_restore("CameraFix", &mut self.camera_fix)
        }
    }

    unsafe fn set_camera_shake_fix_enabled(&mut self, enabled: bool) -> bool {
        let Some(base) = self.ensure_base("CameraShakeFix") else {
            return self.camera_shake_fix.enabled;
        };

        if enabled {
            if self.camera_shake_fix.enabled {
                return true;
            }

            ensure_snapshots(base, &mut self.camera_shake_fix, CAMERA_SHAKE_FIX_REGIONS);
            apply_camera_shake_patch(base);
            self.camera_shake_fix.enabled = true;
            log_info("camera", "CameraShakeFix runtime state: ON");
            true
        } else {
            disable_feature_with_restore("CameraShakeFix", &mut self.camera_shake_fix)
        }
    }

    unsafe fn set_fov_enabled(&mut self, enabled: bool) -> bool {
        let Some(base) = self.ensure_base("FOV") else {
            return self.fov.enabled;
        };

        if enabled {
            if self.fov.enabled {
                return true;
            }

            ensure_snapshots(base, &mut self.fov, FOV_REGIONS);
            if apply_fov_multiplier_hook(base, self.fov_multiplier) {
                self.fov.enabled = true;
                log_info("fov", "FOV runtime state: ON");
                true
            } else {
                log_warn("fov", "Failed to enable FOV runtime hook");
                false
            }
        } else {
            disable_feature_with_restore("FOV", &mut self.fov)
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimePatchPanelState {
    pub(crate) anti_tamper_enabled: bool,
    pub(crate) dlc_fix_enabled: bool,
    pub(crate) camera_fix_enabled: bool,
    pub(crate) camera_shake_fix_enabled: bool,
    pub(crate) fov_enabled: bool,
    pub(crate) fov_multiplier: f32,
}

fn sanitize_fov_multiplier(multiplier: f32, fallback: f32) -> f32 {
    if !multiplier.is_finite() || multiplier <= 0.0 {
        return fallback.max(0.1);
    }

    multiplier.clamp(0.1, 4.0)
}

unsafe fn capture_regions(
    base: usize,
    regions: &[(usize, usize, &str)],
) -> Vec<RegionSnapshot> {
    let mut snapshots = Vec::with_capacity(regions.len());

    for (offset, len, tag) in regions.iter().copied() {
        let address = base + offset;
        let source = std::slice::from_raw_parts(address as *const u8, len);
        snapshots.push(RegionSnapshot {
            address,
            bytes: source.to_vec(),
            flush_tag: tag.to_string(),
        });
    }

    snapshots
}

unsafe fn restore_regions(snapshots: &[RegionSnapshot]) {
    for snapshot in snapshots {
        patch_bytes(snapshot.address, &snapshot.bytes);
        flush_region(snapshot.address, snapshot.bytes.len(), &snapshot.flush_tag);
    }
}

unsafe fn ensure_snapshots(
    base: usize,
    state: &mut ToggleState,
    regions: &[(usize, usize, &str)],
) {
    if state.snapshots.is_none() {
        state.snapshots = Some(capture_regions(base, regions));
    }
}

unsafe fn disable_feature_with_restore(feature_name: &str, state: &mut ToggleState) -> bool {
    if !state.enabled {
        return false;
    }

    if let Some(snapshots) = state.snapshots.as_ref() {
        restore_regions(snapshots);
        state.enabled = false;
        log_info("runtime", &format!("{feature_name} runtime state: OFF"));
        false
    } else {
        log_warn(
            "runtime",
            &format!("{feature_name}: no restore snapshot available; keeping state ON"),
        );
        true
    }
}

fn controller() -> &'static Mutex<RuntimePatchController> {
    static INSTANCE: OnceLock<Mutex<RuntimePatchController>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(RuntimePatchController::new()))
}

fn controller_lock() -> MutexGuard<'static, RuntimePatchController> {
    match controller().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log_warn("runtime", "Runtime patch controller lock was poisoned; recovering state");
            poisoned.into_inner()
        }
    }
}

fn sync_overlay_state(controller: &RuntimePatchController) {
    let panel_state = controller.panel_state();
    set_overlay_panel_feature_state(
        panel_state.anti_tamper_enabled,
        panel_state.dlc_fix_enabled,
        panel_state.camera_fix_enabled,
        panel_state.camera_shake_fix_enabled,
        panel_state.fov_enabled,
        panel_state.fov_multiplier,
    );
}

pub(crate) fn initialize_runtime_patches(base: usize, config: PatchConfig) -> usize {
    let mut controller = controller_lock();
    controller.set_base(base);
    controller.set_fov_multiplier(config.fov_multiplier);

    let mut enabled_groups = 0usize;

    if config.fov_enabled {
        if unsafe { controller.set_fov_enabled(true) } {
            enabled_groups += 1;
        } else {
            log_warn("fov", "FOV multiplier hook was not applied");
        }
    } else {
        log_info("fov", "FOV.Enabled=0, skipping FOV multiplier hook");
    }

    if unsafe { controller.set_anti_tamper_enabled(true) } {
        enabled_groups += 1;
    }

    if config.dlc_car_dealer_fix_enabled {
        if unsafe { controller.set_dlc_fix_enabled(true) } {
            enabled_groups += 1;
        }
    } else {
        log_info(
            "dlc",
            "DlcCarDealerFixEnabled=0, skipping DLC car dealer patch group",
        );
    }

    if config.camera_fix_enabled {
        if unsafe { controller.set_camera_fix_enabled(true) } {
            enabled_groups += 1;
        }
    } else {
        log_info("camera", "CameraFixEnabled=0, skipping camera-fix patch group");
    }

    if config.camera_shake_fix_enabled {
        let _ = unsafe { controller.set_camera_shake_fix_enabled(true) };
    } else {
        log_info(
            "camera",
            "CameraShakeFixEnabled=0, skipping exterior camera shake fix patch",
        );
    }

    sync_overlay_state(&controller);
    enabled_groups
}

pub(crate) fn runtime_patch_panel_state() -> RuntimePatchPanelState {
    let controller = controller_lock();
    controller.panel_state()
}

pub(crate) fn set_runtime_dlc_fix_enabled(enabled: bool) -> bool {
    let mut controller = controller_lock();
    let actual = unsafe { controller.set_dlc_fix_enabled(enabled) };
    sync_overlay_state(&controller);
    actual
}

pub(crate) fn set_runtime_camera_fix_enabled(enabled: bool) -> bool {
    let mut controller = controller_lock();
    let actual = unsafe { controller.set_camera_fix_enabled(enabled) };
    sync_overlay_state(&controller);
    actual
}

pub(crate) fn set_runtime_camera_shake_fix_enabled(enabled: bool) -> bool {
    let mut controller = controller_lock();
    let actual = unsafe { controller.set_camera_shake_fix_enabled(enabled) };
    sync_overlay_state(&controller);
    actual
}

pub(crate) fn set_runtime_fov_enabled(enabled: bool) -> bool {
    let mut controller = controller_lock();
    let actual = unsafe { controller.set_fov_enabled(enabled) };
    sync_overlay_state(&controller);
    actual
}

pub(crate) fn set_runtime_fov_multiplier(multiplier: f32) -> f32 {
    let mut controller = controller_lock();
    let applied = controller.set_fov_multiplier(multiplier);
    sync_overlay_state(&controller);
    applied
}

pub(crate) fn persist_runtime_panel_options() -> bool {
    let mut config = load_patch_config();
    let controller = controller_lock();

    config.anti_tamper_enabled = true;
    config.dlc_car_dealer_fix_enabled = controller.dlc_fix.enabled;
    config.camera_fix_enabled = controller.camera_fix.enabled;
    config.camera_shake_fix_enabled = controller.camera_shake_fix.enabled;
    config.fov_enabled = controller.fov.enabled;
    config.fov_multiplier = controller.fov_multiplier;

    save_patch_config(config)
}

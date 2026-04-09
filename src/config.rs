use std::fs;

use crate::runtime_log::{log_error, log_info, log_warn};

const CONFIG_FILE_NAME: &str = "tdu2-runtime-patch.ini";
const DEFAULT_STARTUP_DELAY_SECONDS: u64 = 3;
const DEFAULT_FOV_ENABLED: bool = true;
const DEFAULT_FOV_MULTIPLIER: f32 = 1.2;

#[derive(Clone, Copy)]
pub(crate) struct PatchConfig {
    pub(crate) anti_tamper_enabled: bool,
    pub(crate) skip_intro_enabled: bool,
    pub(crate) camera_fix_enabled: bool,
    pub(crate) camera_shake_fix_enabled: bool,
    pub(crate) startup_delay_seconds: u64,
    pub(crate) fov_enabled: bool,
    pub(crate) fov_multiplier: f32,
}

impl Default for PatchConfig {
    fn default() -> Self {
        Self {
            anti_tamper_enabled: true,
            skip_intro_enabled: true,
            camera_fix_enabled: true,
            camera_shake_fix_enabled: true,
            startup_delay_seconds: DEFAULT_STARTUP_DELAY_SECONDS,
            fov_enabled: DEFAULT_FOV_ENABLED,
            fov_multiplier: DEFAULT_FOV_MULTIPLIER,
        }
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_f32(raw: &str) -> Option<f32> {
    raw.trim().parse::<f32>().ok()
}

fn write_default_config_file() {
    let defaults = PatchConfig::default();
    let anti_tamper = if defaults.anti_tamper_enabled { 1 } else { 0 };
    let skip_intro = if defaults.skip_intro_enabled { 1 } else { 0 };
    let camera_fix = if defaults.camera_fix_enabled { 1 } else { 0 };
    let camera_shake = if defaults.camera_shake_fix_enabled {
        1
    } else {
        0
    };
    let fov_enabled = if defaults.fov_enabled { 1 } else { 0 };

    let template = format!(
        "[Patch]\nAntiTamperEnabled = {anti_tamper}\nSkipIntroEnabled = {skip_intro}\nCameraFixEnabled = {camera_fix}\nCameraShakeFixEnabled = {camera_shake}\nStartupDelaySeconds = {}\n\n[FOV]\nEnabled = {fov_enabled}\nMultiplier = {:.1}\n",
        defaults.startup_delay_seconds,
        defaults.fov_multiplier
    );

    match fs::write(CONFIG_FILE_NAME, template) {
        Ok(_) => log_info(
            "config",
            &format!("Created default config file: {CONFIG_FILE_NAME}"),
        ),
        Err(err) => log_error(
            "config",
            &format!("Failed to create default config file {CONFIG_FILE_NAME}: {err}"),
        ),
    }
}

pub(crate) fn load_patch_config() -> PatchConfig {
    let mut config = PatchConfig::default();

    let content = match fs::read_to_string(CONFIG_FILE_NAME) {
        Ok(content) => content,
        Err(err) => {
            log_warn(
                "config",
                &format!("Config file read failed ({CONFIG_FILE_NAME}): {err}. Using defaults."),
            );
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
                    log_warn(
                        "config",
                        &format!(
                            "Invalid bool for AntiTamperEnabled on line {}: {value}",
                            line_idx + 1
                        ),
                    );
                }
            }
            "patch.skipintroenabled" | "skipintroenabled" | "patch.skipintro" | "skipintro" => {
                if let Some(parsed) = parse_bool(value) {
                    config.skip_intro_enabled = parsed;
                } else {
                    log_warn(
                        "config",
                        &format!(
                            "Invalid bool for SkipIntroEnabled on line {}: {value}",
                            line_idx + 1
                        ),
                    );
                }
            }
            "patch.camerafixenabled" | "camerafixenabled" => {
                if let Some(parsed) = parse_bool(value) {
                    config.camera_fix_enabled = parsed;
                } else {
                    log_warn(
                        "config",
                        &format!(
                            "Invalid bool for CameraFixEnabled on line {}: {value}",
                            line_idx + 1
                        ),
                    );
                }
            }
            "patch.camerashakefixenabled"
            | "camerashakefixenabled"
            | "patch.exteriorcamerashakefixenabled"
            | "exteriorcamerashakefixenabled"
            | "patch.offroadcamerashakefixenabled"
            | "offroadcamerashakefixenabled" => {
                if let Some(parsed) = parse_bool(value) {
                    config.camera_shake_fix_enabled = parsed;
                } else {
                    log_warn(
                        "config",
                        &format!(
                            "Invalid bool for CameraShakeFixEnabled on line {}: {value}",
                            line_idx + 1
                        ),
                    );
                }
            }
            "patch.startupdelayseconds" | "startupdelayseconds" => {
                if let Ok(parsed) = value.parse::<u64>() {
                    config.startup_delay_seconds = parsed;
                } else {
                    log_warn(
                        "config",
                        &format!(
                            "Invalid integer for StartupDelaySeconds on line {}: {value}",
                            line_idx + 1
                        ),
                    );
                }
            }
            "fov.multiplier" | "fov.mult" | "fovmultiplier" | "patch.fovmultiplier" => {
                if let Some(parsed) = parse_f32(value) {
                    if parsed.is_finite() && parsed > 0.0 {
                        config.fov_multiplier = parsed;
                    } else {
                        log_warn(
                            "config",
                            &format!(
                                "Invalid float range for FOV multiplier on line {}: {value} (must be finite and > 0)",
                                line_idx + 1
                            ),
                        );
                    }
                } else {
                    log_warn(
                        "config",
                        &format!("Invalid float for FOV multiplier on line {}: {value}", line_idx + 1),
                    );
                }
            }
            "fov.enabled" | "fovenabled" | "patch.fovenabled" => {
                if let Some(parsed) = parse_bool(value) {
                    config.fov_enabled = parsed;
                } else {
                    log_warn(
                        "config",
                        &format!("Invalid bool for FOV enabled on line {}: {value}", line_idx + 1),
                    );
                }
            }
            _ => {}
        }
    }

    log_info(
        "config",
        &format!(
            "Config loaded: AntiTamperEnabled={}, SkipIntroEnabled={}, CameraFixEnabled={}, CameraShakeFixEnabled={}, StartupDelaySeconds={}, FOVEnabled={}, FOVMultiplier={:.3}",
            config.anti_tamper_enabled,
            config.skip_intro_enabled,
            config.camera_fix_enabled,
            config.camera_shake_fix_enabled,
            config.startup_delay_seconds,
            config.fov_enabled,
            config.fov_multiplier
        ),
    );

    config
}

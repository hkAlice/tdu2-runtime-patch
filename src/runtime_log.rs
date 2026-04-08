use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Once;

use log::LevelFilter;

const LOG_FILE_NAME: &str = "tdu2-runtime-patch.log";
const PROJECT_NAME: &str = env!("CARGO_PKG_NAME");
const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");
static LOGGER_INIT: Once = Once::new();

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

pub(crate) fn log_info(target: &'static str, message: &str) {
    init_logger_once();
    log::info!(target: target, "{}", message);
}

pub(crate) fn log_warn(target: &'static str, message: &str) {
    init_logger_once();
    log::warn!(target: target, "{}", message);
}

pub(crate) fn log_error(target: &'static str, message: &str) {
    init_logger_once();
    log::error!(target: target, "{}", message);
}

pub(crate) fn log_line(message: &str) {
    log_info("runtime", message);
}

pub(crate) fn log_runtime_banner() {
    let git_hash = option_env!("GIT_COMMIT_HASH").unwrap_or("unknown");
    log_info(
        "runtime",
        &format!("{PROJECT_NAME} v{PROJECT_VERSION} (git {git_hash})"),
    );
}

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=GIT_COMMIT_HASH");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    if let Ok(explicit_hash) = std::env::var("GIT_COMMIT_HASH") {
        let trimmed = explicit_hash.trim();
        if !trimmed.is_empty() {
            println!("cargo:rustc-env=GIT_COMMIT_HASH={trimmed}");
            return;
        }
    }

    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_HASH={hash}");
}

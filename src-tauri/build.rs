#[cfg(target_os = "macos")]
fn macos_swift_runtime_rpaths() -> Vec<String> {
    use std::{env, path::PathBuf, process::Command};

    let mut candidates = Vec::new();

    if let Ok(developer_dir) = env::var("DEVELOPER_DIR") {
        candidates.push(PathBuf::from(developer_dir));
    }

    if let Ok(output) = Command::new("xcode-select").arg("-p").output() {
        if output.status.success() {
            let developer_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !developer_dir.is_empty() {
                candidates.push(PathBuf::from(developer_dir));
            }
        }
    }

    candidates.push(PathBuf::from("/Applications/Xcode.app/Contents/Developer"));

    let mut rpaths = Vec::new();
    for developer_dir in candidates {
        let candidate =
            developer_dir.join("Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx");
        if candidate.join("libswift_Concurrency.dylib").is_file() {
            let candidate = candidate.to_string_lossy().to_string();
            if !rpaths.contains(&candidate) {
                rpaths.push(candidate);
            }
        }
    }

    rpaths
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
        for rpath in macos_swift_runtime_rpaths() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{rpath}");
        }
    }

    tauri_build::build()
}

use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

const REMOTE_BUILD_FILES: &[&str] = &[
    "proxyd/Cargo.toml",
    "proxyd/Cargo.lock",
    "proxyd/src/main.rs",
    "src/auth.rs",
    "src/models.rs",
    "src/proxy_daemon.rs",
    "src/proxy_service.rs",
    "src/state.rs",
    "src/store.rs",
    "src/usage.rs",
    "src/utils.rs",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for path in REMOTE_BUILD_FILES {
        println!("cargo:rerun-if-changed={path}");
    }

    sync_remote_build_resources().expect("failed to prepare remote build resources");
    tauri_build::build()
}

fn sync_remote_build_resources() -> io::Result<()> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"));
    let resource_root = manifest_dir.join("gen").join("remote-build");
    if resource_root.exists() {
        fs::remove_dir_all(&resource_root)?;
    }

    for relative_path in REMOTE_BUILD_FILES {
        copy_file_into_dir(&manifest_dir, &resource_root, relative_path)?;
    }

    Ok(())
}

fn copy_file_into_dir(
    source_root: &Path,
    destination_root: &Path,
    relative_path: &str,
) -> io::Result<()> {
    let source_path = source_root.join(relative_path);
    let destination_path = destination_root.join(relative_path);
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source_path, destination_path)?;
    Ok(())
}

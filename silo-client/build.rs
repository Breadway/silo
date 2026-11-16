use std::{path::Path, process::Command};

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ui_dir = Path::new(&manifest_dir).parent().unwrap().join("silo-ui");

    println!("cargo:rerun-if-changed={}", ui_dir.join("src").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("package.json").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("vite.config.js").display());

    // Skip the build if dist/ already exists and this is not a clean rebuild.
    // Set SILO_FORCE_UI_BUILD=1 to override.
    let dist = ui_dir.join("dist");
    if dist.exists() && std::env::var("SILO_FORCE_UI_BUILD").is_err() {
        return;
    }

    let node_modules = ui_dir.join("node_modules");
    if !node_modules.exists() {
        let status = Command::new("npm")
            .args(["install"])
            .current_dir(&ui_dir)
            .status()
            .expect("npm install failed — ensure Node.js is installed");
        assert!(status.success(), "npm install exited with error");
    }

    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&ui_dir)
        .status()
        .expect("npm run build failed");
    assert!(status.success(), "npm run build exited with error");
}

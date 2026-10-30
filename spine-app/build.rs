fn main() {
    // Rebuild when the interface changes (it is embedded in Spine.exe).
    println!("cargo:rerun-if-changed=ui");
    tauri_build::build()
}

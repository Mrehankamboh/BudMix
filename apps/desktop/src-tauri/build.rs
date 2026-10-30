fn main() {
    // tauri-build does not watch this file, so a new logo would stay out of the exe.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build()
}

const COMMANDS: &[&str] = &["writeSensitiveText", "readText", "clear"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}

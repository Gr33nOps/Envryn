// Hide the console window on Windows release builds. A vault that opens a
// terminal alongside it looks like something that should not be trusted.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    envryn_lib::run()
}

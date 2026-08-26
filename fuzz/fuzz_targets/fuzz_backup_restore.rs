#![no_main]

use libfuzzer_sys::fuzz_target;
use zeroize::Zeroizing;

// Real target: `backup::restore` is what parses an on-disk `.envrynbk` file
// chosen by the user through `backup_restore`'s free-text path field
// (src-tauri/src/ipc.rs) -- the one place this app parses a whole file
// format from outside its own database. A hostile or merely corrupted backup
// file must fail cleanly, never panic, and never allocate unboundedly.
use envryn_core::backup::restore;

fuzz_target!(|data: &[u8]| {
    let password = Zeroizing::new("fuzz-password".to_string());
    let _ = restore(data, &password);
});

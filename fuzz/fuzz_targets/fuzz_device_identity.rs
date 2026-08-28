#![no_main]

use envryn_core::sync::identity::fuzz_parse_identity;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    fuzz_parse_identity(data);
});

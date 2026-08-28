#![no_main]

use envryn_core::sync::protocol::fuzz_parse_wire_message;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    fuzz_parse_wire_message(data);
});

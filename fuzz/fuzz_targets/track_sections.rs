#![no_main]
use libfuzzer_sys::fuzz_target;

#[path = "../targets.rs"]
mod targets;

fuzz_target!(|data: &[u8]| {
    targets::track_sections(data);
});

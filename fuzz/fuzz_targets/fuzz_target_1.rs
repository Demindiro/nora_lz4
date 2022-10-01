#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    nora_lz4::block::decompress(data, 1 << 21);
});

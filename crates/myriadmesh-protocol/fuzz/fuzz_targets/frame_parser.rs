#![no_main]

use libfuzzer_sys::fuzz_target;
use myriadmesh_protocol::frame::Frame;

fuzz_target!(|data: &[u8]| {
    // SECURITY P1.4.1: Fuzz the Frame parser
    // This test ensures the parser doesn't crash on malformed input

    // Try to deserialize the frame from arbitrary bytes
    // Should NOT panic, even on invalid input
    if let Ok(frame) = Frame::deserialize(data) {
        // If we get valid data, try to serialize it back
        let serialized = frame.serialize();
        // Try to parse the serialized form
        let _ = Frame::deserialize(&serialized);

        // Also test validation
        let _ = frame.validate();
    }
});

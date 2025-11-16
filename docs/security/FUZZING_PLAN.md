# Phase 6 Fuzzing Infrastructure Plan

**Document Date**: 2025-11-16
**Status**: 📋 PLANNING - Ready for Implementation
**Target Start**: Week 2-3 of Phase 6
**Target Completion**: Week 5 of Phase 6

---

## Overview

This document describes the fuzzing strategy for Phase 6 security hardening. Fuzzing will systematically test input handling across critical components to identify potential crashes, panics, or unexpected behavior.

---

## Part 1: Fuzzing Framework Selection

### Primary Framework: cargo-fuzz (libFuzzer)

**Rationale**:
- Native Rust support via libFuzzer
- No external dependencies
- Corpus generation and minimization
- Integrates well with Rust build system
- Used by many high-profile Rust projects

**Alternative**: proptest (property-based testing)
- Good for unit-level fuzzing
- Can be combined with cargo-fuzz
- Better for logic verification than crash finding

**Decision**: Use both approaches
- **cargo-fuzz**: For protocol parsers, network inputs (crash finding)
- **proptest**: For algorithmic correctness (logic verification)

---

## Part 2: Target Components for Fuzzing

### Priority 1: Protocol Message Parser

**Component**: `crates/myriadmesh-protocol/src/frame.rs`

**Why Critical**:
- First point of contact for all network messages
- Parses untrusted data from network
- Integer overflow in length fields could cause issues
- Buffer overflow in deserialization

**Test Cases**:
- [ ] Minimal valid message (1 byte)
- [ ] Maximum message (boundary testing)
- [ ] Invalid CRC32 checksums
- [ ] Corrupted headers
- [ ] Out-of-bounds offsets
- [ ] Random binary input
- [ ] Truncated messages
- [ ] Oversized payloads

**Expected Outcome**: No panics, all errors handled gracefully

**Setup**:
```bash
cd crates/myriadmesh-protocol
cargo install cargo-fuzz 2>/dev/null || true
cargo fuzz init
# Add fuzzing targets to fuzz/fuzz_targets/
```

---

### Priority 2: DHT Operations

**Component**: `crates/myriadmesh-dht/src/operations.rs`

**Why Critical**:
- Handles RPC operations from peers
- Storage operations with arbitrary keys/values
- Peer list parsing from untrusted nodes

**Test Cases**:
- [ ] Invalid RPC operation codes
- [ ] Malformed NodeID values (wrong length)
- [ ] Out-of-bounds storage keys
- [ ] Oversized values (> 1MB)
- [ ] Invalid peer lists
- [ ] Missing required fields
- [ ] Integer overflows in bucket calculations

**Expected Outcome**: No panics, graceful error handling

---

### Priority 3: Network Adapter Inputs

**Component**: `crates/myriadmesh-network/src/adapter.rs`

**Why Critical**:
- Each adapter receives data from hardware
- Hardware may send corrupted/malformed packets
- Adapters should never crash on bad input

**Test Cases**:
- [ ] Truncated packets
- [ ] Oversized frames (buffer overflow)
- [ ] Invalid length indicators
- [ ] Out-of-order fragments
- [ ] Random binary data
- [ ] Repeated corrupt frames
- [ ] Mixed valid and invalid packets

**Expected Outcome**: Graceful recovery, no crashes

---

### Priority 4: Routing Decisions

**Component**: `crates/myriadmesh-routing/src/router.rs`

**Why Critical**:
- Makes forwarding decisions based on routing tables
- Could loop infinitely if TTL not decremented
- Path validation must be robust

**Test Cases**:
- [ ] Invalid routing table entries
- [ ] Circular path references
- [ ] Out-of-bounds hop counts
- [ ] Invalid destination NodeIDs
- [ ] Malformed metric values
- [ ] Simultaneous route updates

**Expected Outcome**: No infinite loops, safe routing decisions

---

### Priority 5: Message Storage

**Component**: `crates/myriadmesh-ledger/src/storage.rs`

**Why Critical**:
- Writes to persistent storage
- Data corruption could be permanent
- Must handle disk errors gracefully

**Test Cases**:
- [ ] Corrupted block data
- [ ] Incomplete writes
- [ ] Invalid metadata
- [ ] Oversized entries
- [ ] Transaction conflicts
- [ ] Orphaned record patterns

**Expected Outcome**: Data remains consistent, recoverable

---

## Part 3: Setting Up cargo-fuzz

### Installation Steps

```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# For each target crate, initialize fuzzing
cd crates/myriadmesh-protocol
cargo fuzz init
```

### Fuzzing Target Template

Create `crates/myriadmesh-protocol/fuzz/fuzz_targets/frame_parser.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use myriadmesh_protocol::frame::Frame;

fuzz_target!(|data: &[u8]| {
    // This fuzzes the Frame parser
    // Should NOT panic on any input
    let _ = Frame::from_bytes(data);
});
```

### Running Fuzzing

```bash
# Run for 1000 iterations
cargo fuzz run frame_parser -- -max_len=65536 -runs=1000

# Run with specific corpus
cargo fuzz run frame_parser fuzz/corpus/frame_parser/

# Debug a specific crash
cargo fuzz run frame_parser artifacts/frame_parser/crash-*
```

---

## Part 4: proptest for Algorithmic Verification

### Use Cases

1. **Route Calculation**: Verify shortest path invariants
2. **DHT Operations**: Verify bucket assignments are correct
3. **Message Serialization**: Verify round-trip (serialize → deserialize)
4. **Hash Functions**: Verify collision properties

### Example: Message Round-Trip

```rust
use proptest::prelude::*;
use myriadmesh_protocol::message::Message;

proptest! {
    #[test]
    fn message_roundtrip(msg in any::<Message>()) {
        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: Message = bincode::deserialize(&serialized).unwrap();
        prop_assert_eq!(msg, deserialized);
    }
}
```

---

## Part 5: Fuzzing Infrastructure Setup Timeline

### Week 2-3: Framework Setup
- [ ] Install cargo-fuzz and proptest
- [ ] Create fuzzing directory structure
- [ ] Write fuzzing targets for Priority 1-2 components
- [ ] Setup CI integration

### Week 4-5: Initial Fuzzing Runs
- [ ] Run fuzzers for 10,000+ iterations each
- [ ] Document any crashes or panics found
- [ ] Create issues for findings
- [ ] Fix critical issues found

### Week 6: Extended Fuzzing
- [ ] Run long-duration fuzzing (100,000+ iterations)
- [ ] Add corpus from real-world testing
- [ ] Verify fixes have not regressed

---

## Part 6: CI/CD Integration

### GitHub Actions Workflow

Create `.github/workflows/fuzzing.yml`:

```yaml
name: Fuzzing Checks

on:
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM UTC
  workflow_dispatch:

jobs:
  fuzz:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target:
          - frame_parser
          - dht_operations
          - routing_decisions
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz
      - name: Run fuzzing (${{ matrix.target }})
        working-directory: crates/myriadmesh-protocol
        run: cargo fuzz run ${{ matrix.target }} -- -max_len=65536 -timeout=10 -runs=10000
```

---

## Part 7: Results Tracking

### Fuzzing Result Logs

Create directory: `docs/security/fuzzing_results/`

For each fuzzing run:
- Date and fuzzer version
- Target component
- Number of iterations
- Any crashes/panics found
- Performance metrics

### Example Log Entry

```
Date: 2025-11-23
Fuzzer: cargo-fuzz v0.11
Target: frame_parser
Iterations: 10,000
Runtime: 42 seconds
Crashes Found: 0
Panics Found: 0
Corpus Size: 156 entries
Status: ✅ PASS
```

---

## Part 8: Crash Minimization

### When a Crash is Found

1. **Preserve the input**: Save minimized crash input
2. **Reproduce locally**: `cargo fuzz run target artifacts/crash-*`
3. **Create issue**: Document the crash with reproduction steps
4. **Fix and test**: Write unit test for the fix
5. **Verify**: Re-run fuzzer to confirm fix

### Crash Artifact Organization

```
crates/myriadmesh-protocol/
└── artifacts/
    ├── frame_parser/
    │   ├── crash-20251120-frame-overflow
    │   ├── crash-20251120-invalid-header
    │   └── slow-20251121-parser-timeout
    └── routing/
        └── crash-20251122-infinite-loop
```

---

## Part 9: Expected Outcomes

### Success Criteria

- [ ] All Priority 1-2 fuzzers run 10,000+ iterations without crashes
- [ ] Any crashes found have documented fixes
- [ ] Corpus generated from fuzzing is incorporated into test suite
- [ ] No performance regressions from fixes
- [ ] Fuzzing infrastructure integrated into CI

### Metrics to Track

- **Iterations per second**: Performance baseline
- **Corpus coverage**: Code paths covered by fuzzing
- **Crash detection time**: When first crash found
- **Fix regression**: Re-fuzzing after fix confirms no new crashes

---

## Part 10: Long-Term Fuzzing

### Post-Phase-6 Recommendations

1. **Continuous Fuzzing**: Keep fuzzing targets running in CI
2. **Corpus Management**: Archive and version control fuzzed inputs
3. **Coverage Analysis**: Use code coverage tools with fuzzing
4. **Continuous Integration**: Fail builds if crashes found
5. **Benchmark Tracking**: Monitor fuzzing performance over time

---

## References

### Fuzzing Tools
- cargo-fuzz: https://github.com/rust-fuzz/cargo-fuzz
- libFuzzer: https://llvm.org/docs/LibFuzzer/
- proptest: https://docs.rs/proptest/

### Security Resources
- OWASP Fuzzing: https://owasp.org/www-community/attacks/Fuzzing
- CWE-680: Integer Overflow to Buffer Overflow
- CWE-190: Integer Overflow or Wraparound

---

**Document Status**: Ready for Implementation
**Owner**: P1 Security Work Stream
**Target Completion**: Week 5, Phase 6

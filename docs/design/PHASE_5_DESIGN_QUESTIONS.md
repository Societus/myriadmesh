# Phase 5 Design Clarification Questions

**Document:** MyriadMesh Phase 5 Framework Review
**Date:** 2025-11-15
**Status:** Awaiting User Input

This document contains critical design decisions needed to guide Phase 5 implementation. Please provide your preferences and constraints for each question below.

---

## Question 1: Adapter Implementation Priority & Hardware Availability

### Context
Phase 5 includes 6 new adapter types with varying maturity levels and hardware requirements:

| Adapter | Hardware Availability | Community Demand | Implementation Complexity |
|---------|----------------------|-----------------|--------------------------|
| LoRaWAN | HIGH (common) | HIGH | MEDIUM |
| WiFi HaLoW | LOW (dev boards only) | MEDIUM | MEDIUM |
| APRS | MEDIUM (TNC modules) | HIGH | MEDIUM |
| FRS/GMRS | MEDIUM (commercial radios) | MEDIUM | HIGH |
| CB/HF Radio | MEDIUM (amateur equipment) | MEDIUM | HIGH |
| Dial-up | LOW (legacy) | LOW | LOW |

### Questions

**1a.** Which adapters should have **full working implementations** (not stubs) in Phase 5?
- ☐ LoRaWAN (most prioritized)
- ☐ APRS (amateur radio network)
- ☐ FRS/GMRS (local radio mesh)
- ☐ WiFi HaLoW (long-range WiFi)
- ☐ CB/HF Radio (long-distance HF)
- ☐ Dial-up (emergency fallback)

**1b.** For adapters without full hardware, should we:
- A) Implement software simulators/mocks for testing
- B) Document hardware requirements only (defer implementation)
- C) Implement partial support (TX/RX stubs, no actual HW interface)

**1c.** Should we establish a **"Reference Hardware" list** (specific radio models, TNC devices, modems) to test against?

---

## Question 2: License Verification & Regulatory Compliance

### Context
Three adapters require operator licenses:
- **APRS:** Ham Radio license (FCC)
- **HF Radio:** Amateur Radio license (FCC)
- **GMRS:** GMRS license (FCC, not required for FRS-only)

### Questions

**2a.** For adapters requiring licenses, what should the check do?

**Option A - Strict (Blocks Usage):**
```rust
if !verify_license(callsign) {
    return Err(AdapterError::LicenseNotFound)
}
```
- ✅ Prevents unlicensed operation
- ❌ Requires FCC database access (may not be available offline)

**Option B - Permissive (Warning Only):**
```rust
if !verify_license(callsign) {
    warn!("License not verified: {}", callsign);
}
```
- ✅ Works offline
- ❌ Cannot prevent unlicensed operation

**Option C - Configurable (User Choice):**
```rust
if config.license_check && !verify_license(callsign) {
    // Option A or B behavior
}
```
- ✅ Flexible for different use cases
- ❌ More complex configuration

**2b.** Should we include a **local FCC callsign database** in the codebase?
- A) Yes, with weekly updates via HTTP
- B) No, require user to configure/verify manually
- C) Optional (user-provided list)

**2c.** Should we add special handling for **emergency operations** (e.g., waiving license checks during disaster response)?

---

## Question 3: Fragmentation & Reassembly Strategy

### Context
Low-MTU adapters (LoRa, APRS, FRS/GMRS) require message fragmentation. Currently, the design proposes handling this at:

**Option A - Adapter Level:**
- Each adapter fragments/reassembles its own messages
- Pros: Adapter-specific optimization
- Cons: Duplicated code, harder to test

**Option B - Message Router Level:**
- Central fragmentation before handing to adapter
- Pros: Unified implementation
- Cons: Less adapter flexibility

**Option C - Hybrid:**
- Small messages (<MTU): Adapter handles directly
- Large messages (>MTU): Router fragments first

### Questions

**3a.** Where should fragmentation/reassembly happen?
- ☐ Adapter level
- ☐ Message router level
- ☐ Hybrid approach

**3b.** How should we handle **reassembly timeouts** (fragments arriving slowly)?
- A) 60 seconds (current proposal)
- B) Configurable per adapter
- C) Adaptive based on adapter latency

**3c.** Should we support **out-of-order fragments**?
- Yes (more complex but faster)
- No (sequential only, simpler)

---

## Question 4: Power Management & Battery Optimization

### Context
Several Phase 5 adapters are battery-powered (LoRa, FRS/GMRS):
- Battery devices should have lower routing priority when AC-powered alternatives exist
- Power-aware applications need to estimate battery life
- Some adapters support sleep modes (e.g., WiFi HaLoW TWT)

### Questions

**4a.** Should we implement **power-aware routing**?
- A) Yes - deprioritize high-power adapters in route selection
- B) No - let application layer handle power decisions
- C) Only as a hint/advisory, not hard constraint

**4b.** What power states should adapters support?
- ☐ Active (fully operational)
- ☐ Idle (RX only, low power)
- ☐ Sleep (RX disabled, timer-based wake)
- ☐ Off (powered down)

**4c.** Should adapters report **estimated battery hours**?
- A) Yes, mandatory (required for power-aware decisions)
- B) Yes, optional (nice to have)
- C) No, too complex

**4d.** Should we enforce **data usage limits** on cellular/dial-up adapters?
- A) Hard limits (block transmission when exceeded)
- B) Soft limits (warn but allow)
- C) No automatic limits

---

## Question 5: Propagation Awareness for HF Radio

### Context
HF (shortwave) communication depends heavily on ionospheric conditions:
- Solar Flux Index (SFI) - affects maximum usable frequency
- K-index - geomagnetic activity (disrupts propagation)
- Time of day - affects which bands work
- Season - 11-year solar cycle

Current proposal: Optional auto-band-switching based on space weather.

### Questions

**5a.** Should HF adapter include **propagation forecasting**?
- A) Yes - query space weather API and auto-select bands
- B) No - static frequency configuration only
- C) Optional (configurable per deployment)

**5b.** If yes, what data sources should we use?
- ☐ NOAA Space Weather Prediction Center API
- ☐ Local cached database (requires manual updates)
- ☐ VOACAP online prediction service

**5c.** How should propagation affect routing decisions?
- A) Consider SFI/K-index in hop costs
- B) Suggest alternative adapters if HF propagation poor
- C) Only informational (for user/app awareness)

**5d.** Should we support **multiple HF frequencies** with automatic failover?
- Yes (complex but more resilient)
- No (single frequency per deployment)

---

## Question 6: Legacy Technology & Sunset Policy

### Context
Some Phase 5 adapters are **legacy** (dial-up, shortwave):
- Dial-up modems are obsolete (last common use ~2005)
- HF radio is used but declining (amateur radio population aging)
- APRS is stable but specialized (ham radio only)

### Questions

**6a.** Should Phase 5 **prioritize legacy adapters** or focus on future-facing tech?
- A) Full support for all (including dial-up)
- B) Core adapters only (LoRa, APRS, FRS/GMRS)
- C) Legacy adapters as optional plugins (not core codebase)

**6b.** What **sunset policy** should apply to legacy adapters?
- A) No sunset (support indefinitely)
- B) Sunset in Phase 6 (finalize then freeze)
- C) Deprecation warning in Phase 5 (plan removal)
- D) Remove (focus on modern tech)

**6c.** Should dial-up include **SMS fallback** (GSM SMS as last resort)?
- A) Yes - implement SMS transport
- B) No - too niche, skip
- C) Yes but defer to Phase 6

---

## Question 7: Testing & Simulation Strategy

### Context
Some Phase 5 adapters are difficult to test without hardware:
- WiFi HaLoW: Very few devices available
- HF Radio: Requires expensive CAT-compatible radio
- Dial-up: No modern modems or ISPs

### Questions

**7a.** Should we implement **mock adapters** for hardware-less testing?
- A) Yes, comprehensive mocks for all adapters
- B) Mocks for complex adapters (HF, APRS) only
- C) No, document hardware requirements only

**7b.** Should integration tests require **actual hardware**?
- A) Yes - physical devices required for CI/CD
- B) No - use simulators/mocks only
- C) Both - full tests with hardware, fast tests with mocks

**7c.** Should we set up a **community hardware lab**?
- A) Yes - maintain test devices for developers
- B) No - too costly
- C) Maybe - if community donates equipment

---

## Question 8: Protocol Compatibility & Interoperability

### Context
Some Phase 5 adapters have existing protocol ecosystems:
- **Meshtastic:** Popular LoRa mesh firmware (Python/Android)
- **APRS:** Existing TNC/KISS infrastructure
- **FRS/GMRS:** Commercial radio ecosystem

Should MyriadMesh be able to **interoperate** with these ecosystems?

### Questions

**8a.** Should Phase 5 include **Meshtastic bridge/compatibility**?
- A) Yes - implement Meshtastic packet format
- B) No - MyriadMesh protocol only
- C) Optional module (plugin)

**8b.** Should APRS adapter work with **existing APRS-IS network**?
- A) Yes - enable worldwide APRS-IS connectivity
- B) No - local RF-only
- C) Both with toggle

**8c.** For commercial radio adapters (FRS/GMRS, HF), what level of compatibility?
- A) Interop with other MyriadMesh nodes only
- B) Capability to work with other vendors' equipment
- C) Full protocol translation

---

## Question 9: Documentation & User Accessibility

### Context
Phase 5 includes complex technologies (HF radio, Meshtastic, APRS) that require specialized knowledge.

### Questions

**9a.** What level of documentation should Phase 5 have?
- A) Comprehensive guides for each adapter (setup, troubleshooting, licensing)
- B) API documentation only (configuration parameters)
- C) Quick-start guides + API docs

**9b.** Should we create **setup guides for popular platforms**?
- ☐ LoRa with Heltec/TTGO boards
- ☐ APRS with TNC232 or Direwolf
- ☐ FRS/GMRS with Baofeng UV-5R
- ☐ HF with FT-991A, IC-7300
- ☐ All the above

**9c.** Should Phase 5 include **troubleshooting/diagnostics tools**?
- A) Yes - CLI tools to test adapters individually
- B) No - documentation only
- C) Yes, but defer to Phase 6

---

## Question 10: Future Expansion & Extensibility

### Context
Phase 5 may not cover all future radio technologies. How should we design for extensibility?

### Questions

**10a.** Should Phase 5 define an **adapter plugin system**?
- A) Yes - allow community to contribute adapters
- B) No - core adapters only
- C) Phase 6 feature

**10b.** Should we include **LoRa v2.0 or LoRaWAN 1.1 support**?
- A) Yes - future-proof the adapter
- B) No - LoRaWAN 1.0 is sufficient
- C) Basic support, extensible for v1.1

**10c.** Should we reserve adapter type IDs for **future technologies**?
- A) Yes - define slots for mesh networks, satellites, etc.
- B) No - add as needed
- C) Maybe - reserve a few for upcoming

---

## Implementation Impact Matrix

This table shows how your answers affect timeline and complexity:

| Question | Answer A | Answer B | Answer C |
|----------|----------|----------|----------|
| **1a** (Priorities) | All implemented ↑↑ cost | Core only ↑ cost | Balanced ↕ cost |
| **2a** (License) | Requires DB ↑ complex | Offline ↓ simple | Configurable ↑ cost |
| **3a** (Fragmentation) | Duplication ↑ maint | Central ↑ robust | Hybrid ↑↑ complex |
| **4a** (Power routing) | ↑↑ complex | ↓ simple | ↑ moderate |
| **5a** (HF propagation) | ↑↑ complex | ↓ simple | ↑ moderate |
| **6a** (Legacy) | ↑↑ effort | ↑ effort | ↓ effort |
| **7a** (Testing) | ↑↑ mocks | ↑ mocks | ↑ maintenance |
| **8a** (Meshtastic) | ↑ complexity | ↓ simple | ↑ maintenance |
| **9a** (Documentation) | ↑↑ hours | ↓ simple | ↑ hours |
| **10a** (Plugins) | ↑↑ architecture | ↓ simple | ↑ future-ready |

---

## Summary Form

Please provide answers to the following questions:

```
Question 1a: [ ] LoRa [ ] APRS [ ] FRS/GMRS [ ] HF [ ] WiFi HaLoW [ ] Dialup
Question 1b: [ ] Simulators [ ] Docs only [ ] Partial stubs
Question 1c: [ ] Yes [ ] No

Question 2a: [ ] Option A [ ] Option B [ ] Option C
Question 2b: [ ] Yes [ ] No [ ] Optional
Question 2c: [ ] Yes [ ] No

Question 3a: [ ] Adapter [ ] Router [ ] Hybrid
Question 3b: [ ] 60s [ ] Configurable [ ] Adaptive
Question 3c: [ ] Yes [ ] No

Question 4a: [ ] Yes [ ] No [ ] Advisory
Question 4b: [ ] Active [ ] Idle [ ] Sleep [ ] Off
Question 4c: [ ] Yes mandatory [ ] Yes optional [ ] No
Question 4d: [ ] Hard [ ] Soft [ ] No

Question 5a: [ ] Yes [ ] No [ ] Optional
Question 5b: [ ] NOAA [ ] Local DB [ ] VOACAP
Question 5c: [ ] Routing [ ] Suggestions [ ] Informational
Question 5d: [ ] Yes [ ] No

Question 6a: [ ] Full support [ ] Core only [ ] Optional plugins
Question 6b: [ ] No sunset [ ] Phase 6 [ ] Deprecation [ ] Remove
Question 6c: [ ] Yes [ ] No [ ] Phase 6

Question 7a: [ ] Comprehensive [ ] Complex only [ ] None
Question 7b: [ ] Hardware [ ] Simulators [ ] Both
Question 7c: [ ] Yes [ ] No [ ] Maybe

Question 8a: [ ] Yes [ ] No [ ] Optional
Question 8b: [ ] Yes [ ] No [ ] Both
Question 8c: [ ] Local only [ ] Compatible [ ] Full translation

Question 9a: [ ] Comprehensive [ ] API only [ ] Quick-start
Question 9b: [ ] Check all applicable [ ]
Question 9c: [ ] Yes [ ] No [ ] Phase 6

Question 10a: [ ] Yes [ ] No [ ] Phase 6
Question 10b: [ ] Yes [ ] No [ ] Basic + extensible
Question 10c: [ ] Yes [ ] No [ ] Maybe
```

---

## Next Steps

1. **Review** the Phase 5 design framework document
2. **Provide answers** to these clarification questions
3. **We will** adjust implementation priorities and timelines based on your input
4. **Begin** Phase 5 implementation with focused scope

---

**Contact:** For questions about this framework, please refer to:
- `/home/user/myriadmesh/docs/design/phase5-detailed-design.md` - Full design document
- `/home/user/myriadmesh/crates/myriadmesh-network/src/adapters/` - Stub implementations


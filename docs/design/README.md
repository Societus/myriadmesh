# MyriadMesh Design Documents

This directory contains detailed design documents for each phase of the MyriadMesh project.

## Phase 2: Core Protocol

**Version**: 2.0 (Updated with Comprehensive Privacy Protections)
**Status**: Ready for Review

### Documents

1. **[Executive Summary](./phase2-executive-summary.md)** ⭐ **START HERE**
   - Quick overview of key design decisions
   - Open questions that need your input
   - 5-minute read

2. **[Privacy Protections](./phase2-privacy-protections.md)** 🔒 **IMPORTANT**
   - Comprehensive privacy strategy (8 layers)
   - Network-adaptive privacy with user transparency
   - Protection against malicious relay surveillance
   - 15-minute read

3. **[Detailed Design](./phase2-detailed-design.md)**
   - Complete technical specification
   - Architecture diagrams
   - Code examples
   - Testing strategy
   - 30-minute read

### Key Design Decisions

- ✅ **No Proof-of-Work** (optimized for embedded devices)
- ✅ **E2E Encryption** with optional content tags for relay filtering
- ✅ **Reputation-based** Sybil resistance
- ✅ **Availability-first** protocol with security-first principles for designated sensitive traffic
- ✅ **Strict resource limits** to prevent DoS
- ✅ **NEW: Comprehensive privacy protections** (8 layers, network-adaptive)

### Your Input Needed

Please review the executive summary and provide feedback on:

1. **Content tagging system** - Does it meet your requirements?
2. **DHT bootstrap strategy** - How should new nodes join?
3. **Storage replication factor** - k=3 or k=20?
4. **i2p integration timeline** - Phase 2 or Phase 4?
5. **Overall design approval** - Ready to implement?

### Timeline

**Estimated**: 14 weeks (~3.5 months)
- Weeks 1-2: DHT Implementation
- Weeks 3-4: Message Router
- Weeks 5-6: Network Abstraction Layer
- Weeks 7-8: Ethernet Adapter
- **Week 9: Privacy Layer Integration** (NEW)
- **Week 10: Onion Routing** (NEW)
- Weeks 11-12: Integration & Testing (expanded scope)
- Weeks 13-14: Security Review & Hardening (expanded scope)

---

## How to Provide Feedback

1. Read the [Executive Summary](./phase2-executive-summary.md)
2. Answer the open questions
3. Review the [Detailed Design](./phase2-detailed-design.md) (optional, for deep dive)
4. Provide any additional feedback or concerns

---

## Next Steps

After your review and approval:
1. Begin Phase 2 implementation (Week 1: DHT)
2. Regular progress updates
3. Iterative testing and refinement

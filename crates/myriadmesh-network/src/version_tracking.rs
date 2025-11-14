//! Component version tracking and reputation impact
//!
//! Tracks adapter library versions and applies reputation penalties
//! for outdated or vulnerable components.

use crate::types::AdapterCapabilities;
use myriadmesh_crypto::signing::Signature;
use myriadmesh_protocol::{types::AdapterType, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Semantic version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl SemanticVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse from string like "1.2.3"
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }
}

/// Component version manifest for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentManifest {
    /// Node ID
    pub node_id: NodeId,

    /// Manifest creation timestamp
    pub created_at: u64,

    /// Core MyriadMesh version
    pub core_version: SemanticVersion,

    /// Adapter versions
    pub adapters: HashMap<AdapterType, AdapterVersionInfo>,

    /// Security advisory compliance
    pub security_advisories: Vec<AdvisoryCompliance>,

    /// Ed25519 signature of manifest (optional for now)
    #[serde(skip)]
    pub signature: Option<Signature>,
}

/// Version information for a specific adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterVersionInfo {
    /// Adapter type
    pub adapter_type: AdapterType,

    /// Library name (e.g., "btleplug", "modemmanager")
    pub library: String,

    /// Current version
    pub version: SemanticVersion,

    /// Latest available version (if known)
    pub latest_version: Option<SemanticVersion>,

    /// Days since last update
    pub days_since_update: u32,

    /// Known CVEs affecting this version
    pub known_cves: Vec<CveInfo>,

    /// Component status
    pub status: AdapterComponentStatus,
}

/// Status of an adapter component
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterComponentStatus {
    /// Up to date
    Current,

    /// Minor update available (non-security)
    MinorUpdate,

    /// Security update available
    SecurityUpdate,

    /// Critical security update available
    CriticalUpdate,

    /// Version deprecated by maintainers
    Deprecated,

    /// Version no longer supported
    Unsupported,
}

/// CVE information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveInfo {
    /// CVE identifier (e.g., "CVE-2024-1234")
    pub cve_id: String,

    /// Severity level
    pub severity: CveSeverity,

    /// CVSS score (0.0-10.0)
    pub cvss_score: f32,

    /// Version that patches this CVE
    pub patched_in: SemanticVersion,

    /// Description
    pub description: String,
}

/// CVE severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CveSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Security advisory compliance status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvisoryCompliance {
    /// Advisory ID
    pub advisory_id: String,

    /// Whether node is compliant
    pub compliant: bool,

    /// Components affected
    pub affected_components: Vec<AdapterType>,

    /// Remediation deadline (if any)
    pub deadline: Option<u64>,
}

/// Calculate reputation penalty for outdated components
///
/// Returns a penalty factor between 0.0 (no penalty) and 0.95 (maximum penalty)
pub fn calculate_version_penalty(manifest: &ComponentManifest) -> f64 {
    let mut penalty = 0.0;

    for (_adapter_type, info) in &manifest.adapters {
        // Base penalty by status
        let status_penalty = match info.status {
            AdapterComponentStatus::Current => 0.0,
            AdapterComponentStatus::MinorUpdate => {
                // Light penalty that increases with age
                0.01 * (info.days_since_update as f64 / 30.0).min(5.0)
            }
            AdapterComponentStatus::SecurityUpdate => {
                // Moderate penalty that increases with time
                0.10 * (1.0 + info.days_since_update as f64 / 7.0)
            }
            AdapterComponentStatus::CriticalUpdate => {
                // Heavy penalty that increases rapidly
                0.30 * (1.0 + info.days_since_update as f64 / 3.0)
            }
            AdapterComponentStatus::Deprecated => {
                // Severe fixed penalty
                0.50
            }
            AdapterComponentStatus::Unsupported => {
                // Maximum fixed penalty
                1.00
            }
        };

        penalty += status_penalty;

        // Additional penalty for known CVEs
        for cve in &info.known_cves {
            let cve_base_penalty = match cve.severity {
                CveSeverity::Low => 0.05,
                CveSeverity::Medium => 0.15,
                CveSeverity::High => 0.40,
                CveSeverity::Critical => 0.80,
            };

            // Penalty increases with time unpatched
            let days_unpatched = info.days_since_update;
            let time_multiplier = 1.0 + (days_unpatched as f64 / 7.0).min(10.0);

            penalty += cve_base_penalty * time_multiplier;
        }
    }

    // Security advisory non-compliance
    for advisory in &manifest.security_advisories {
        if !advisory.compliant {
            // Check if past deadline
            if let Some(deadline) = advisory.deadline {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if now > deadline {
                    // Heavy penalty for missing deadline
                    let days_overdue = ((now - deadline) / 86400) as f64;
                    penalty += 0.25 * (1.0 + days_overdue / 7.0);
                } else {
                    // Moderate penalty before deadline
                    penalty += 0.10;
                }
            } else {
                penalty += 0.10;
            }
        }
    }

    // Cap penalty at 0.95 to leave minimum 5% reputation
    penalty.min(0.95)
}

impl ComponentManifest {
    /// Create a new manifest for this node
    pub fn new(node_id: NodeId, core_version: SemanticVersion) -> Self {
        Self {
            node_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            core_version,
            adapters: HashMap::new(),
            security_advisories: Vec::new(),
            signature: None,
        }
    }

    /// Add adapter version information
    pub fn add_adapter(&mut self, info: AdapterVersionInfo) {
        self.adapters.insert(info.adapter_type, info);
    }

    /// Get reputation penalty for this manifest
    pub fn get_reputation_penalty(&self) -> f64 {
        calculate_version_penalty(self)
    }

    /// Check if any adapters have critical updates available
    pub fn has_critical_updates(&self) -> bool {
        self.adapters
            .values()
            .any(|info| info.status == AdapterComponentStatus::CriticalUpdate)
    }

    /// Check if any adapters are unsupported
    pub fn has_unsupported_components(&self) -> bool {
        self.adapters
            .values()
            .any(|info| info.status == AdapterComponentStatus::Unsupported)
    }

    /// Get list of CVEs affecting this node
    pub fn get_all_cves(&self) -> Vec<&CveInfo> {
        self.adapters
            .values()
            .flat_map(|info| &info.known_cves)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_version_ordering() {
        let v1 = SemanticVersion::new(1, 2, 3);
        let v2 = SemanticVersion::new(1, 2, 4);
        let v3 = SemanticVersion::new(1, 3, 0);

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_semantic_version_parse() {
        let v = SemanticVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);

        assert!(SemanticVersion::parse("invalid").is_none());
        assert!(SemanticVersion::parse("1.2").is_none());
    }

    #[test]
    fn test_penalty_current_version() {
        let mut manifest = ComponentManifest::new(
            NodeId::from_bytes([0u8; 64]),
            SemanticVersion::new(1, 0, 0),
        );

        manifest.add_adapter(AdapterVersionInfo {
            adapter_type: AdapterType::Ethernet,
            library: "tokio".to_string(),
            version: SemanticVersion::new(1, 0, 0),
            latest_version: Some(SemanticVersion::new(1, 0, 0)),
            days_since_update: 0,
            known_cves: vec![],
            status: AdapterComponentStatus::Current,
        });

        let penalty = calculate_version_penalty(&manifest);
        assert_eq!(penalty, 0.0);
    }

    #[test]
    fn test_penalty_minor_update() {
        let mut manifest = ComponentManifest::new(
            NodeId::from_bytes([0u8; 64]),
            SemanticVersion::new(1, 0, 0),
        );

        manifest.add_adapter(AdapterVersionInfo {
            adapter_type: AdapterType::Ethernet,
            library: "tokio".to_string(),
            version: SemanticVersion::new(1, 0, 0),
            latest_version: Some(SemanticVersion::new(1, 0, 1)),
            days_since_update: 30,
            known_cves: vec![],
            status: AdapterComponentStatus::MinorUpdate,
        });

        let penalty = calculate_version_penalty(&manifest);
        assert!(penalty > 0.0 && penalty < 0.1);
    }

    #[test]
    fn test_penalty_critical_cve() {
        let mut manifest = ComponentManifest::new(
            NodeId::from_bytes([0u8; 64]),
            SemanticVersion::new(1, 0, 0),
        );

        manifest.add_adapter(AdapterVersionInfo {
            adapter_type: AdapterType::Bluetooth,
            library: "btleplug".to_string(),
            version: SemanticVersion::new(0, 10, 0),
            latest_version: Some(SemanticVersion::new(0, 11, 0)),
            days_since_update: 14,
            known_cves: vec![CveInfo {
                cve_id: "CVE-2024-1234".to_string(),
                severity: CveSeverity::Critical,
                cvss_score: 9.8,
                patched_in: SemanticVersion::new(0, 11, 0),
                description: "Critical RCE vulnerability".to_string(),
            }],
            status: AdapterComponentStatus::CriticalUpdate,
        });

        let penalty = calculate_version_penalty(&manifest);
        // Should have high penalty: 0.30 * (1 + 14/3) + 0.80 * (1 + 14/7)
        assert!(penalty > 0.5);
    }

    #[test]
    fn test_penalty_unsupported() {
        let mut manifest = ComponentManifest::new(
            NodeId::from_bytes([0u8; 64]),
            SemanticVersion::new(1, 0, 0),
        );

        manifest.add_adapter(AdapterVersionInfo {
            adapter_type: AdapterType::Cellular,
            library: "old-modem".to_string(),
            version: SemanticVersion::new(0, 1, 0),
            latest_version: None,
            days_since_update: 365,
            known_cves: vec![],
            status: AdapterComponentStatus::Unsupported,
        });

        let penalty = calculate_version_penalty(&manifest);
        assert!(penalty >= 0.95); // Should be capped
    }

    #[test]
    fn test_has_critical_updates() {
        let mut manifest = ComponentManifest::new(
            NodeId::from_bytes([0u8; 64]),
            SemanticVersion::new(1, 0, 0),
        );

        assert!(!manifest.has_critical_updates());

        manifest.add_adapter(AdapterVersionInfo {
            adapter_type: AdapterType::Bluetooth,
            library: "btleplug".to_string(),
            version: SemanticVersion::new(0, 10, 0),
            latest_version: Some(SemanticVersion::new(0, 11, 0)),
            days_since_update: 7,
            known_cves: vec![],
            status: AdapterComponentStatus::CriticalUpdate,
        });

        assert!(manifest.has_critical_updates());
    }
}

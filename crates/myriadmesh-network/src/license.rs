//! License Management for Radio Adapters
//!
//! Provides license verification and enforcement for regulated radio adapters
//! (APRS, HF Radio, GMRS). Blocks transmission without valid license but allows
//! receive operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::error::{NetworkError, Result};

/// Amateur radio license class
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmateurClass {
    /// Technician class (entry level, VHF/UHF privileges)
    Technician,
    /// General class (some HF privileges)
    General,
    /// Extra class (full privileges)
    Extra,
}

impl AmateurClass {
    /// Check if this license class allows HF operation
    pub fn allows_hf(&self) -> bool {
        matches!(self, Self::General | Self::Extra)
    }

    /// Check if this license class allows VHF/UHF operation
    pub fn allows_vhf_uhf(&self) -> bool {
        true // All classes allow VHF/UHF
    }
}

/// License class for radio operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LicenseClass {
    /// Amateur (Ham) Radio license
    Amateur(AmateurClass),
    /// GMRS (General Mobile Radio Service) license
    GMRS,
    /// CB (Citizens Band) - no license required in most jurisdictions
    CB,
}

/// License state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LicenseState {
    /// Valid license with callsign
    Valid {
        /// Radio callsign (e.g., "N0CALL")
        callsign: String,
        /// License class
        license_class: LicenseClass,
        /// Optional expiration timestamp (Unix time)
        expires_at: Option<u64>,
    },
    /// No license configured
    None,
    /// License has expired
    Expired {
        /// Expired callsign
        callsign: String,
    },
}

impl LicenseState {
    /// Check if license is valid for transmission
    pub fn can_transmit(&self) -> bool {
        matches!(self, Self::Valid { .. })
    }

    /// Check if license allows HF operation
    pub fn allows_hf(&self) -> bool {
        match self {
            Self::Valid { license_class: LicenseClass::Amateur(class), .. } => class.allows_hf(),
            _ => false,
        }
    }

    /// Get callsign if available
    pub fn callsign(&self) -> Option<&str> {
        match self {
            Self::Valid { callsign, .. } | Self::Expired { callsign } => Some(callsign),
            Self::None => None,
        }
    }
}

/// Cache entry for license validation
#[derive(Debug, Clone)]
struct CacheEntry {
    callsign: String,
    valid: bool,
    cached_at: u64,
}

/// FCC license database client (for online validation)
pub struct FccClient {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    cache_ttl: u64, // Cache time-to-live in seconds
}

impl FccClient {
    /// Create a new FCC client
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: 86400, // 24 hours
        }
    }

    /// Validate a callsign against FCC database
    ///
    /// This is a placeholder implementation. In production, this would query
    /// the FCC ULS database API or a local mirror.
    pub async fn validate_callsign(&self, callsign: &str) -> Result<bool> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(callsign) {
                let now = now();
                if now < entry.cached_at + self.cache_ttl {
                    return Ok(entry.valid);
                }
            }
        }

        // In production: Query FCC ULS database
        // For now, validate format only
        let valid = Self::validate_callsign_format(callsign);

        // Cache result
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                callsign.to_string(),
                CacheEntry {
                    callsign: callsign.to_string(),
                    valid,
                    cached_at: now(),
                },
            );
        }

        Ok(valid)
    }

    /// Validate callsign format (basic validation)
    fn validate_callsign_format(callsign: &str) -> bool {
        // Basic format: 1-2 letters, 1 digit, 1-3 letters/digits
        // Examples: K1ABC, N0CALL, W9XYZ
        let parts: Vec<&str> = callsign.split('-').collect();
        let base = parts[0];

        if base.len() < 3 || base.len() > 6 {
            return false;
        }

        // Check pattern
        let chars: Vec<char> = base.chars().collect();
        let has_digit = chars.iter().any(|c| c.is_ascii_digit());
        let has_letter = chars.iter().any(|c| c.is_ascii_alphabetic());

        has_digit && has_letter
    }
}

impl Default for FccClient {
    fn default() -> Self {
        Self::new()
    }
}

/// License manager for radio adapters
pub struct LicenseManager {
    state: Arc<RwLock<LicenseState>>,
    fcc_client: Option<Arc<FccClient>>,
}

impl LicenseManager {
    /// Create a new license manager
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(LicenseState::None)),
            fcc_client: Some(Arc::new(FccClient::new())),
        }
    }

    /// Create a license manager without FCC validation (offline mode)
    pub fn new_offline() -> Self {
        Self {
            state: Arc::new(RwLock::new(LicenseState::None)),
            fcc_client: None,
        }
    }

    /// Set license for this node
    pub async fn set_license(
        &self,
        callsign: String,
        class: LicenseClass,
        expires_at: Option<u64>,
    ) -> Result<()> {
        // Validate with FCC if client is available
        if let Some(ref fcc) = self.fcc_client {
            let valid = fcc.validate_callsign(&callsign).await?;
            if !valid {
                return Err(NetworkError::InvalidCallsign(callsign));
            }
        }

        // Check expiration
        if let Some(expiration) = expires_at {
            if now() >= expiration {
                let mut state = self.state.write().await;
                *state = LicenseState::Expired {
                    callsign: callsign.clone(),
                };
                return Err(NetworkError::LicenseExpired(callsign));
            }
        }

        // Set license
        let mut state = self.state.write().await;
        *state = LicenseState::Valid {
            callsign,
            license_class: class,
            expires_at,
        };

        Ok(())
    }

    /// Check if transmission is allowed
    pub async fn can_transmit(&self) -> Result<()> {
        let state = self.state.read().await;
        match &*state {
            LicenseState::Valid { expires_at, callsign, .. } => {
                // Check expiration
                if let Some(expiration) = expires_at {
                    if now() >= *expiration {
                        return Err(NetworkError::LicenseExpired(callsign.clone()));
                    }
                }
                Ok(())
            }
            LicenseState::None => Err(NetworkError::LicenseRequired),
            LicenseState::Expired { callsign } => {
                Err(NetworkError::LicenseExpired(callsign.clone()))
            }
        }
    }

    /// Check if receive is allowed (always true)
    pub fn can_receive(&self) -> Result<()> {
        Ok(()) // Always allowed to listen
    }

    /// Get current license state
    pub async fn get_license(&self) -> LicenseState {
        self.state.read().await.clone()
    }

    /// Clear license
    pub async fn clear_license(&self) {
        let mut state = self.state.write().await;
        *state = LicenseState::None;
    }

    /// Check if HF operation is allowed
    pub async fn can_operate_hf(&self) -> bool {
        self.state.read().await.allows_hf()
    }

    /// Get callsign if available
    pub async fn get_callsign(&self) -> Option<String> {
        self.state.read().await.callsign().map(|s| s.to_string())
    }
}

impl Default for LicenseManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amateur_class_privileges() {
        assert!(AmateurClass::Technician.allows_vhf_uhf());
        assert!(!AmateurClass::Technician.allows_hf());

        assert!(AmateurClass::General.allows_vhf_uhf());
        assert!(AmateurClass::General.allows_hf());

        assert!(AmateurClass::Extra.allows_vhf_uhf());
        assert!(AmateurClass::Extra.allows_hf());
    }

    #[test]
    fn test_license_state_can_transmit() {
        let valid = LicenseState::Valid {
            callsign: "N0CALL".to_string(),
            license_class: LicenseClass::Amateur(AmateurClass::General),
            expires_at: None,
        };
        assert!(valid.can_transmit());

        let none = LicenseState::None;
        assert!(!none.can_transmit());

        let expired = LicenseState::Expired {
            callsign: "N0CALL".to_string(),
        };
        assert!(!expired.can_transmit());
    }

    #[test]
    fn test_callsign_format_validation() {
        assert!(FccClient::validate_callsign_format("K1ABC"));
        assert!(FccClient::validate_callsign_format("N0CALL"));
        assert!(FccClient::validate_callsign_format("W9XYZ"));
        assert!(FccClient::validate_callsign_format("AA1A"));

        assert!(!FccClient::validate_callsign_format("ABC"));
        assert!(!FccClient::validate_callsign_format("123"));
        assert!(!FccClient::validate_callsign_format("A"));
    }

    #[tokio::test]
    async fn test_license_manager_set_and_check() {
        let manager = LicenseManager::new_offline();

        // Initially no license
        assert!(manager.can_transmit().await.is_err());

        // Set license
        manager
            .set_license(
                "N0CALL".to_string(),
                LicenseClass::Amateur(AmateurClass::General),
                None,
            )
            .await
            .unwrap();

        // Now can transmit
        assert!(manager.can_transmit().await.is_ok());

        // Can receive (always allowed)
        assert!(manager.can_receive().is_ok());

        // HF allowed for General class
        assert!(manager.can_operate_hf().await);
    }

    #[tokio::test]
    async fn test_license_manager_expiration() {
        let manager = LicenseManager::new_offline();

        // Set expired license
        let past = now() - 1000;
        let result = manager
            .set_license(
                "N0CALL".to_string(),
                LicenseClass::Amateur(AmateurClass::General),
                Some(past),
            )
            .await;

        assert!(result.is_err());

        // Check state is expired
        let state = manager.get_license().await;
        assert!(matches!(state, LicenseState::Expired { .. }));
    }

    #[tokio::test]
    async fn test_fcc_client_cache() {
        let client = FccClient::new();

        // First call - not cached
        let valid1 = client.validate_callsign("N0CALL").await.unwrap();

        // Second call - should hit cache
        let valid2 = client.validate_callsign("N0CALL").await.unwrap();

        assert_eq!(valid1, valid2);
    }
}

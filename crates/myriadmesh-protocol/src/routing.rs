//! Routing flags and content tags for Phase 2
//!
//! This module defines routing metadata for messages, including:
//! - Routing flags (E2E_STRICT, SENSITIVE, RELAY_FILTERABLE, etc.)
//! - Content tag system for optional relay filtering
//! - Privacy-aware routing policies

use serde::{Deserialize, Serialize};

/// Maximum number of content tags per message
pub const MAX_CONTENT_TAGS: usize = 10;

/// Maximum length of a content tag in bytes
pub const MAX_TAG_LENGTH: usize = 32;

/// Routing flags bitfield
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingFlags(u8);

impl RoutingFlags {
    /// Message is strictly E2E encrypted (default)
    pub const E2E_STRICT: u8 = 0b0000_0001;

    /// User-designated sensitive content (relays MUST forward)
    pub const SENSITIVE: u8 = 0b0000_0010;

    /// Relays MAY use content tags for filtering
    pub const RELAY_FILTERABLE: u8 = 0b0000_0100;

    /// Request multi-path routing (future)
    pub const MULTI_PATH: u8 = 0b0000_1000;

    /// Message is anonymous (route via i2p) (Phase 4)
    pub const ANONYMOUS: u8 = 0b0001_0000;

    /// Sender opts out of onion routing (privacy reduction)
    pub const NO_ONION_ROUTING: u8 = 0b0010_0000;

    /// Create new routing flags
    pub fn new(flags: u8) -> Self {
        RoutingFlags(flags)
    }

    /// Get default flags (E2E_STRICT)
    pub fn default() -> Self {
        RoutingFlags(Self::E2E_STRICT)
    }

    /// Check if flag is set
    pub fn contains(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    /// Set a flag
    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    /// Clear a flag
    pub fn clear(&mut self, flag: u8) {
        self.0 &= !flag;
    }

    /// Get raw value
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl Default for RoutingFlags {
    fn default() -> Self {
        Self::default()
    }
}

/// Content tag for optional relay filtering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentTag(String);

impl ContentTag {
    /// Create a new content tag
    pub fn new(tag: impl Into<String>) -> Result<Self, String> {
        let tag = tag.into();

        if tag.is_empty() {
            return Err("Content tag cannot be empty".to_string());
        }

        if tag.len() > MAX_TAG_LENGTH {
            return Err(format!(
                "Content tag too long: {} bytes (max {})",
                tag.len(),
                MAX_TAG_LENGTH
            ));
        }

        Ok(ContentTag(tag))
    }

    /// Get tag as string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContentTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Standard content tags
pub mod standard_tags {
    /// Content classification
    pub const NSFW: &str = "nsfw";
    pub const POLITICAL: &str = "political";
    pub const COMMERCIAL: &str = "commercial";
    pub const EDUCATIONAL: &str = "educational";

    /// Media types
    pub const MEDIA_IMAGE: &str = "media:image";
    pub const MEDIA_VIDEO: &str = "media:video";
    pub const MEDIA_AUDIO: &str = "media:audio";
    pub const MEDIA_DOCUMENT: &str = "media:document";

    /// Size hints
    pub const SIZE_SMALL: &str = "size:small"; // <10KB
    pub const SIZE_MEDIUM: &str = "size:medium"; // 10KB-1MB
    pub const SIZE_LARGE: &str = "size:large"; // >1MB

    /// Priority hints
    pub const PRIORITY_EMERGENCY: &str = "priority:emergency";
    pub const PRIORITY_HIGH: &str = "priority:high";
    pub const PRIORITY_NORMAL: &str = "priority:normal";
}

/// Relay policy for content filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayPolicy {
    /// Enable filtering based on tags
    pub enable_filtering: bool,

    /// Blocked tags (relay will refuse these)
    pub blocked_tags: Vec<String>,

    /// Allowed tags (if set, only relay these)
    pub allowed_tags: Vec<String>,

    /// Always relay sensitive messages (ignore filtering)
    pub always_relay_sensitive: bool,

    /// Maximum message size to relay (bytes)
    pub max_message_size: usize,

    /// Maximum relay rate (messages per minute)
    pub max_relay_rate: u32,
}

impl Default for RelayPolicy {
    fn default() -> Self {
        RelayPolicy {
            enable_filtering: false, // Default: relay everything
            blocked_tags: Vec::new(),
            allowed_tags: Vec::new(),
            always_relay_sensitive: true,
            max_message_size: 1024 * 1024, // 1MB
            max_relay_rate: 1000,           // 1000 msg/min
        }
    }
}

impl RelayPolicy {
    /// Check if a message should be relayed based on policy
    pub fn should_relay(&self, flags: &RoutingFlags, tags: &[ContentTag]) -> bool {
        // MUST relay if SENSITIVE flag set
        if flags.contains(RoutingFlags::SENSITIVE) && self.always_relay_sensitive {
            return true;
        }

        // If filtering disabled, relay everything
        if !self.enable_filtering {
            return true;
        }

        // If not RELAY_FILTERABLE, relay (no tags to filter on)
        if !flags.contains(RoutingFlags::RELAY_FILTERABLE) {
            return true;
        }

        // Check blocked tags
        for tag in tags {
            if self.blocked_tags.contains(&tag.0) {
                return false; // Refuse relay
            }
        }

        // Check allowed tags (if specified)
        if !self.allowed_tags.is_empty() {
            let has_allowed = tags.iter().any(|tag| self.allowed_tags.contains(&tag.0));
            if !has_allowed {
                return false;
            }
        }

        true // Relay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_flags() {
        let mut flags = RoutingFlags::default();
        assert!(flags.contains(RoutingFlags::E2E_STRICT));
        assert!(!flags.contains(RoutingFlags::SENSITIVE));

        flags.set(RoutingFlags::SENSITIVE);
        assert!(flags.contains(RoutingFlags::SENSITIVE));

        flags.clear(RoutingFlags::SENSITIVE);
        assert!(!flags.contains(RoutingFlags::SENSITIVE));
    }

    #[test]
    fn test_content_tag_creation() {
        let tag = ContentTag::new("nsfw").unwrap();
        assert_eq!(tag.as_str(), "nsfw");

        // Empty tag should fail
        assert!(ContentTag::new("").is_err());

        // Too long tag should fail
        let long_tag = "a".repeat(MAX_TAG_LENGTH + 1);
        assert!(ContentTag::new(long_tag).is_err());
    }

    #[test]
    fn test_relay_policy_sensitive() {
        let policy = RelayPolicy::default();
        let mut flags = RoutingFlags::new(RoutingFlags::SENSITIVE);
        flags.set(RoutingFlags::RELAY_FILTERABLE);

        let tags = vec![ContentTag::new("nsfw").unwrap()];

        // SENSITIVE messages should always be relayed
        assert!(policy.should_relay(&flags, &tags));
    }

    #[test]
    fn test_relay_policy_filtering_disabled() {
        let policy = RelayPolicy::default();
        let flags = RoutingFlags::new(RoutingFlags::RELAY_FILTERABLE);
        let tags = vec![ContentTag::new("nsfw").unwrap()];

        // With filtering disabled, should relay
        assert!(policy.should_relay(&flags, &tags));
    }

    #[test]
    fn test_relay_policy_blocked_tags() {
        let mut policy = RelayPolicy::default();
        policy.enable_filtering = true;
        policy.blocked_tags = vec!["nsfw".to_string()];

        let flags = RoutingFlags::new(RoutingFlags::RELAY_FILTERABLE);
        let tags = vec![ContentTag::new("nsfw").unwrap()];

        // Should refuse relay
        assert!(!policy.should_relay(&flags, &tags));

        // Different tag should relay
        let tags2 = vec![ContentTag::new("educational").unwrap()];
        assert!(policy.should_relay(&flags, &tags2));
    }

    #[test]
    fn test_relay_policy_allowed_tags() {
        let mut policy = RelayPolicy::default();
        policy.enable_filtering = true;
        policy.allowed_tags = vec!["educational".to_string()];

        let flags = RoutingFlags::new(RoutingFlags::RELAY_FILTERABLE);

        // Educational tag should relay
        let tags1 = vec![ContentTag::new("educational").unwrap()];
        assert!(policy.should_relay(&flags, &tags1));

        // Other tags should not relay
        let tags2 = vec![ContentTag::new("nsfw").unwrap()];
        assert!(!policy.should_relay(&flags, &tags2));
    }

    #[test]
    fn test_e2e_strict_not_filterable() {
        let mut policy = RelayPolicy::default();
        policy.enable_filtering = true;
        policy.blocked_tags = vec!["nsfw".to_string()];

        let flags = RoutingFlags::new(RoutingFlags::E2E_STRICT);
        let tags = vec![ContentTag::new("nsfw").unwrap()];

        // E2E_STRICT (not RELAY_FILTERABLE) should relay regardless
        assert!(policy.should_relay(&flags, &tags));
    }
}

//! Routing-related protocol types for Phase 2

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// Routing flags for message routing control
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct RoutingFlags: u8 {
        /// Message is strictly E2E encrypted (default)
        const E2E_STRICT = 0b0000_0001;

        /// User-designated sensitive content (relays MUST forward)
        const SENSITIVE = 0b0000_0010;

        /// Relays MAY use content tags for filtering
        const RELAY_FILTERABLE = 0b0000_0100;

        /// Request multi-path routing (future)
        const MULTI_PATH = 0b0000_1000;

        /// Message is anonymous (route via i2p) (Phase 4)
        const ANONYMOUS = 0b0001_0000;

        /// Sender opts out of onion routing
        const NO_ONION_ROUTING = 0b0010_0000;

        /// Message has been relayed (set by intermediate nodes)
        const RELAYED = 0b0100_0000;
    }
}

impl Default for RoutingFlags {
    fn default() -> Self {
        RoutingFlags::E2E_STRICT
    }
}

/// Content tag for optional relay filtering
/// Format: "category:value" or "flag"
/// Maximum 32 bytes per tag
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentTag(String);

impl ContentTag {
    /// Maximum length of a content tag
    pub const MAX_LENGTH: usize = 32;

    /// Create a new content tag
    pub fn new(tag: impl Into<String>) -> Result<Self, String> {
        let tag = tag.into();
        if tag.len() > Self::MAX_LENGTH {
            return Err(format!(
                "Content tag too long: {} bytes (max {})",
                tag.len(),
                Self::MAX_LENGTH
            ));
        }
        Ok(ContentTag(tag))
    }

    /// Get the tag string
    pub fn as_str(&self) -> &str {
        &self.0
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

/// Message priority (0-255)
/// Higher values = higher priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(u8);

impl Priority {
    /// Background priority (0-63)
    pub const BACKGROUND: Priority = Priority(32);

    /// Low priority (64-127)
    pub const LOW: Priority = Priority(96);

    /// Normal priority (128-191) - Default
    pub const NORMAL: Priority = Priority(160);

    /// High priority (192-223)
    pub const HIGH: Priority = Priority(208);

    /// Emergency priority (224-255)
    pub const EMERGENCY: Priority = Priority(240);

    /// Create a new priority
    pub fn new(value: u8) -> Self {
        Priority(value)
    }

    /// Get the priority value
    pub fn value(&self) -> u8 {
        self.0
    }

    /// Check if this is an emergency priority message
    pub fn is_emergency(&self) -> bool {
        self.0 >= 224
    }

    /// Check if this is a high priority message
    pub fn is_high(&self) -> bool {
        self.0 >= 192
    }

    /// Get priority queue index (0-4)
    pub fn queue_index(&self) -> usize {
        match self.0 {
            0..=63 => 0,    // Background
            64..=127 => 1,  // Low
            128..=191 => 2, // Normal
            192..=223 => 3, // High
            224..=255 => 4, // Emergency
        }
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::NORMAL
    }
}

impl From<u8> for Priority {
    fn from(value: u8) -> Self {
        Priority(value)
    }
}

impl From<Priority> for u8 {
    fn from(priority: Priority) -> Self {
        priority.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_flags() {
        let flags = RoutingFlags::SENSITIVE | RoutingFlags::E2E_STRICT;
        assert!(flags.contains(RoutingFlags::SENSITIVE));
        assert!(flags.contains(RoutingFlags::E2E_STRICT));
        assert!(!flags.contains(RoutingFlags::RELAY_FILTERABLE));
    }

    #[test]
    fn test_content_tag() {
        let tag = ContentTag::new("media:image").unwrap();
        assert_eq!(tag.as_str(), "media:image");

        // Tag too long
        let long_tag = "a".repeat(33);
        assert!(ContentTag::new(long_tag).is_err());
    }

    #[test]
    fn test_priority() {
        assert_eq!(Priority::NORMAL.value(), 160);
        assert_eq!(Priority::EMERGENCY.value(), 240);

        assert!(Priority::EMERGENCY.is_emergency());
        assert!(Priority::HIGH.is_high());
        assert!(!Priority::NORMAL.is_emergency());

        assert_eq!(Priority::BACKGROUND.queue_index(), 0);
        assert_eq!(Priority::NORMAL.queue_index(), 2);
        assert_eq!(Priority::EMERGENCY.queue_index(), 4);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::EMERGENCY > Priority::HIGH);
        assert!(Priority::HIGH > Priority::NORMAL);
        assert!(Priority::NORMAL > Priority::LOW);
        assert!(Priority::LOW > Priority::BACKGROUND);
    }
}

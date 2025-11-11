//! MyriadMesh Core Library
//!
//! This is the main library that ties together the cryptography and protocol modules.

pub use myriadmesh_crypto as crypto;
pub use myriadmesh_protocol as protocol;

pub use crypto::CryptoError;
pub use protocol::ProtocolError;

/// Initialize the MyriadMesh library
pub fn init() -> Result<(), CryptoError> {
    crypto::init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }
}

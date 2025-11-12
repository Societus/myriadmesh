//! SAM (Simple Anonymous Messaging) protocol client for i2p
//!
//! Provides a client implementation for communicating with i2p routers
//! using the SAM v3 protocol.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SamError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("SAM protocol error: {0}")]
    ProtocolError(String),

    #[error("Invalid destination: {0}")]
    InvalidDestination(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SamError>;

/// SAM protocol version
const SAM_VERSION: &str = "3.1";

/// SAM session types
#[derive(Debug, Clone, Copy)]
pub enum SessionStyle {
    /// Stream-based connections (TCP-like)
    Stream,
    /// Datagram-based (UDP-like)
    Datagram,
    /// Raw data forwarding
    Raw,
}

impl SessionStyle {
    fn as_str(&self) -> &str {
        match self {
            SessionStyle::Stream => "STREAM",
            SessionStyle::Datagram => "DATAGRAM",
            SessionStyle::Raw => "RAW",
        }
    }
}

/// I2P destination (base64 encoded public key + certificate)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamDestination {
    pub destination: String,
}

impl SamDestination {
    /// Create new destination from string
    pub fn new(destination: String) -> Self {
        SamDestination { destination }
    }

    /// Get the destination string
    pub fn as_str(&self) -> &str {
        &self.destination
    }
}

/// SAM connection for communicating with i2p router
pub struct SamConnection {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

impl SamConnection {
    /// Connect to SAM bridge
    pub fn connect(sam_addr: &str) -> Result<Self> {
        let stream =
            TcpStream::connect(sam_addr).map_err(|e| SamError::ConnectionFailed(e.to_string()))?;

        stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(30))).ok();

        let reader = BufReader::new(
            stream
                .try_clone()
                .map_err(|e| SamError::ConnectionFailed(e.to_string()))?,
        );

        let mut connection = SamConnection { stream, reader };

        // Send HELLO
        connection.send_command(&format!(
            "HELLO VERSION MIN={} MAX={}\n",
            SAM_VERSION, SAM_VERSION
        ))?;
        let response = connection.read_response()?;

        if !response.starts_with("HELLO REPLY") || !response.contains("RESULT=OK") {
            return Err(SamError::ProtocolError(format!(
                "HELLO failed: {}",
                response
            )));
        }

        Ok(connection)
    }

    /// Generate a new i2p destination
    pub fn generate_destination(&mut self) -> Result<SamDestination> {
        self.send_command("DEST GENERATE\n")?;
        let response = self.read_response()?;

        if !response.starts_with("DEST REPLY") {
            return Err(SamError::ProtocolError(format!(
                "DEST GENERATE failed: {}",
                response
            )));
        }

        // Parse destination from response
        // Format: DEST REPLY PUB=<destination> PRIV=<private_keys>
        let dest = Self::extract_value(&response, "PUB=")
            .ok_or_else(|| SamError::ProtocolError("No destination in response".to_string()))?;

        Ok(SamDestination::new(dest))
    }

    /// Create a SAM session
    pub fn create_session(
        &mut self,
        session_id: &str,
        style: SessionStyle,
        destination: Option<&str>,
    ) -> Result<SamDestination> {
        let dest_str = destination.unwrap_or("TRANSIENT");

        let cmd = format!(
            "SESSION CREATE STYLE={} ID={} DESTINATION={}\n",
            style.as_str(),
            session_id,
            dest_str
        );

        self.send_command(&cmd)?;
        let response = self.read_response()?;

        if !response.starts_with("SESSION STATUS") || !response.contains("RESULT=OK") {
            return Err(SamError::SessionError(format!(
                "Session creation failed: {}",
                response
            )));
        }

        // Extract destination from response
        let dest = Self::extract_value(&response, "DESTINATION=")
            .ok_or_else(|| SamError::ProtocolError("No destination in response".to_string()))?;

        Ok(SamDestination::new(dest))
    }

    /// Connect to a remote i2p destination
    pub fn stream_connect(&mut self, session_id: &str, destination: &str) -> Result<TcpStream> {
        let cmd = format!(
            "STREAM CONNECT ID={} DESTINATION={}\n",
            session_id, destination
        );

        self.send_command(&cmd)?;
        let response = self.read_response()?;

        if !response.starts_with("STREAM STATUS") || !response.contains("RESULT=OK") {
            return Err(SamError::ProtocolError(format!(
                "STREAM CONNECT failed: {}",
                response
            )));
        }

        // Return the underlying stream for data transfer
        self.stream.try_clone().map_err(SamError::IoError)
    }

    /// Accept incoming stream connections
    pub fn stream_accept(&mut self, session_id: &str) -> Result<(TcpStream, SamDestination)> {
        let cmd = format!("STREAM ACCEPT ID={}\n", session_id);

        self.send_command(&cmd)?;
        let response = self.read_response()?;

        if !response.starts_with("STREAM STATUS") || !response.contains("RESULT=OK") {
            return Err(SamError::ProtocolError(format!(
                "STREAM ACCEPT failed: {}",
                response
            )));
        }

        // Extract remote destination
        let remote_dest = Self::extract_value(&response, "DESTINATION=")
            .ok_or_else(|| SamError::ProtocolError("No destination in response".to_string()))?;

        let stream = self.stream.try_clone().map_err(SamError::IoError)?;

        Ok((stream, SamDestination::new(remote_dest)))
    }

    /// Send a command to SAM bridge
    fn send_command(&mut self, command: &str) -> Result<()> {
        self.stream
            .write_all(command.as_bytes())
            .map_err(SamError::IoError)?;
        self.stream.flush().map_err(SamError::IoError)
    }

    /// Read a response from SAM bridge
    fn read_response(&mut self) -> Result<String> {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(SamError::IoError)?;
        Ok(line.trim().to_string())
    }

    /// Extract a value from SAM response
    fn extract_value(response: &str, key: &str) -> Option<String> {
        response.find(key).map(|start| {
            let value_start = start + key.len();
            let remaining = &response[value_start..];

            // Find the end (space or end of string)
            let end = remaining
                .find(' ')
                .or_else(|| remaining.find('\n'))
                .unwrap_or(remaining.len());

            remaining[..end].to_string()
        })
    }
}

/// Helper for managing SAM sessions
pub struct SamSession {
    connection: SamConnection,
    session_id: String,
    destination: SamDestination,
    style: SessionStyle,
}

impl SamSession {
    /// Create a new SAM session
    pub fn create(
        sam_addr: &str,
        session_id: String,
        style: SessionStyle,
        destination: Option<String>,
    ) -> Result<Self> {
        let mut connection = SamConnection::connect(sam_addr)?;

        let dest = connection.create_session(&session_id, style, destination.as_deref())?;

        Ok(SamSession {
            connection,
            session_id,
            destination: dest,
            style,
        })
    }

    /// Get the session's i2p destination
    pub fn destination(&self) -> &SamDestination {
        &self.destination
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Connect to a remote destination (for STREAM sessions)
    pub fn connect(&mut self, destination: &str) -> Result<TcpStream> {
        if !matches!(self.style, SessionStyle::Stream) {
            return Err(SamError::SessionError(
                "STREAM CONNECT only supported for STREAM sessions".to_string(),
            ));
        }

        self.connection
            .stream_connect(&self.session_id, destination)
    }

    /// Accept incoming connection (for STREAM sessions)
    pub fn accept(&mut self) -> Result<(TcpStream, SamDestination)> {
        if !matches!(self.style, SessionStyle::Stream) {
            return Err(SamError::SessionError(
                "STREAM ACCEPT only supported for STREAM sessions".to_string(),
            ));
        }

        self.connection.stream_accept(&self.session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_value() {
        let response = "SESSION STATUS RESULT=OK DESTINATION=abc123xyz";
        let result = SamConnection::extract_value(response, "RESULT=");
        assert_eq!(result, Some("OK".to_string()));

        let dest = SamConnection::extract_value(response, "DESTINATION=");
        assert_eq!(dest, Some("abc123xyz".to_string()));
    }

    #[test]
    fn test_extract_value_with_spaces() {
        let response = "STREAM STATUS RESULT=OK DESTINATION=abc123 MORE=data";
        let dest = SamConnection::extract_value(response, "DESTINATION=");
        assert_eq!(dest, Some("abc123".to_string()));
    }

    #[test]
    fn test_session_style() {
        assert_eq!(SessionStyle::Stream.as_str(), "STREAM");
        assert_eq!(SessionStyle::Datagram.as_str(), "DATAGRAM");
        assert_eq!(SessionStyle::Raw.as_str(), "RAW");
    }

    #[test]
    fn test_destination_creation() {
        let dest = SamDestination::new("test_destination".to_string());
        assert_eq!(dest.as_str(), "test_destination");
    }

    // Integration tests require a running i2p router
    #[test]
    #[ignore]
    fn test_sam_connection() {
        let result = SamConnection::connect("127.0.0.1:7656");
        assert!(result.is_ok());
    }

    #[test]
    #[ignore]
    fn test_generate_destination() {
        let mut conn = SamConnection::connect("127.0.0.1:7656").unwrap();
        let dest = conn.generate_destination();
        assert!(dest.is_ok());
        let dest = dest.unwrap();
        assert!(!dest.destination.is_empty());
    }

    #[test]
    #[ignore]
    fn test_create_session() {
        let result = SamSession::create(
            "127.0.0.1:7656",
            "test_session".to_string(),
            SessionStyle::Stream,
            None,
        );
        assert!(result.is_ok());
    }
}

#[derive(Debug)]
pub enum ProtocolError {
    IoError(std::io::Error),
    DeserializationError(String),
    InvalidFrame(String),
    ConnectionClosed,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::IoError(e) => write!(f, "IO error: {}", e),
            ProtocolError::DeserializationError(e) => write!(f, "Deserialization error: {}", e),
            ProtocolError::InvalidFrame(e) => write!(f, "Invalid frame: {}", e),
            ProtocolError::ConnectionClosed => write!(f, "Connection closed"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<std::io::Error> for ProtocolError {
    fn from(error: std::io::Error) -> Self {
        ProtocolError::IoError(error)
    }
}

impl From<bincode::Error> for ProtocolError {
    fn from(error: bincode::Error) -> Self {
        ProtocolError::DeserializationError(error.to_string())
    }
}

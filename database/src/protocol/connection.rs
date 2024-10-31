use crate::protocol::{error::ProtocolError, response::Response};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn send_raw_command(&mut self, command: &str) -> Result<(), ProtocolError> {
        let mut buffer = Vec::new();
        // Add frame type (2 for RawCommand)
        buffer.push(2);
        // Add length as 4-byte little-endian
        let length = command.len() as u32;
        buffer.extend_from_slice(&length.to_le_bytes());
        // Add the command itself
        buffer.extend_from_slice(command.as_bytes());

        match self.stream.write_all(&buffer) {
            Ok(_) => self.stream.flush()?,
            Err(e) => return Err(ProtocolError::IoError(e)),
        }
        Ok(())
    }

    pub fn receive_response(&mut self) -> Result<Response, ProtocolError> {
        let mut type_buffer = [0u8; 1];
        match self.stream.read_exact(&mut type_buffer) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                return Err(ProtocolError::ConnectionClosed);
            }
            Err(e) => return Err(ProtocolError::IoError(e)),
        }

        let mut length_buffer = [0u8; 4];
        self.stream.read_exact(&mut length_buffer)?;
        let length = u32::from_le_bytes(length_buffer);

        if length > 1024 * 1024 {
            // 1MB limit for safety
            return Err(ProtocolError::InvalidFrame("Response too large".into()));
        }

        let mut payload = vec![0u8; length as usize];
        self.stream.read_exact(&mut payload)?;

        let response: Response = bincode::deserialize(&payload)?;
        Ok(response)
    }

    pub fn send_response(&mut self, response: Response) -> Result<(), ProtocolError> {
        let payload = bincode::serialize(&response)?;
        let mut buffer = Vec::new();
        buffer.push(3); // Response frame type
        buffer.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buffer.extend_from_slice(&payload);

        self.stream.write_all(&buffer)?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive_raw_command(&mut self) -> Result<String, ProtocolError> {
        let mut type_buffer = [0u8; 1];
        match self.stream.read_exact(&mut type_buffer) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                return Err(ProtocolError::ConnectionClosed);
            }
            Err(e) => return Err(ProtocolError::IoError(e)),
        }

        let mut length_buffer = [0u8; 4];
        self.stream.read_exact(&mut length_buffer)?;
        let length = u32::from_le_bytes(length_buffer);

        if length > 1024 * 1024 {
            // 1MB limit for safety
            return Err(ProtocolError::InvalidFrame("Command too large".into()));
        }

        let mut payload = vec![0u8; length as usize];
        self.stream.read_exact(&mut payload)?;

        String::from_utf8(payload).map_err(|e| ProtocolError::DeserializationError(e.to_string()))
    }
}

use crate::storage::Value;
use serde::{Deserialize, Serialize};

pub mod connection;
mod tests;

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Get {
        key: i32,
    },
    Set {
        key: i32,
        value: Value,
    },
    Delete {
        key: i32,
    },
    Update {
        key: i32,
        value: Value,
    },
    All,
    Strlen {
        key: i32,
    },
    Strcat {
        key: i32,
        value: Value,
    },
    Substr {
        key: i32,
        start: usize,
        length: usize,
    },
    Ping,
    Exit,
    Expression(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Value(Option<Value>),
    Range(Vec<(i32, Value)>),
    Error(String),
    Pong,
    Size(usize),
}

#[derive(Debug)]
pub enum FrameType {
    Command,
    RawCommand,
    Response,
}

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


pub struct Frame {
    pub frame_type: FrameType,
    pub length: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(frame_type: FrameType, payload: Vec<u8>) -> Self {
        let length = payload.len() as u32;
        Self { frame_type, length, payload }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        // Add frame type marker
        buffer.push(match self.frame_type {
            FrameType::Command => 1,
            FrameType::RawCommand => 2,
            FrameType::Response => 3,
        });
        // Add length
        buffer.extend_from_slice(&self.length.to_le_bytes());
        // Add payload
        buffer.extend_from_slice(&self.payload);
        buffer
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if data.len() < 5 {  // 1 byte type + 4 bytes length
            return Err("Invalid frame: too short".into());
        }
        
        let frame_type = match data[0] {
            1 => FrameType::Command,
            2 => FrameType::RawCommand,
            3 => FrameType::Response,
            _ => return Err("Invalid frame type".into()),
        };
        
        let length = u32::from_le_bytes(data[1..5].try_into()?);
        let payload = data[5..5 + length as usize].to_vec();
        
        Ok(Self { frame_type, length, payload })
    }
}

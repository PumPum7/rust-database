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

pub struct Frame {
    pub length: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(payload: Vec<u8>) -> Self {
        let length = payload.len() as u32;
        Self { length, payload }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&self.length.to_le_bytes());
        buffer.extend_from_slice(&self.payload);
        buffer
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if data.len() < 4 {
            return Err("Invalid frame: too short".into());
        }
        let length = u32::from_le_bytes(data[0..4].try_into()?);
        let payload = data[4..4 + length as usize].to_vec();
        Ok(Self { length, payload })
    }
}

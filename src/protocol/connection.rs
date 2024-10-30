use super::{Command, Frame, Response};
use bincode;
use std::io::{Read, Write};
use std::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn send_command(&mut self, command: Command) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = bincode::serialize(&command)?;
        let frame = Frame::new(encoded);
        self.stream.write_all(&frame.serialize())?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive_response(&mut self) -> Result<Response, Box<dyn std::error::Error>> {
        let mut length_buffer = [0u8; 4];
        self.stream.read_exact(&mut length_buffer)?;
        let length = u32::from_le_bytes(length_buffer);

        let mut payload_buffer = vec![0u8; length as usize];
        self.stream.read_exact(&mut payload_buffer)?;

        let response: Response = bincode::deserialize(&payload_buffer)?;
        Ok(response)
    }

    pub fn send_response(&mut self, response: Response) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = bincode::serialize(&response)?;
        let frame = Frame::new(encoded);
        self.stream.write_all(&frame.serialize())?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive_command(&mut self) -> Result<Command, Box<dyn std::error::Error>> {
        let mut length_buffer = [0u8; 4];
        self.stream.read_exact(&mut length_buffer)?;
        let length = u32::from_le_bytes(length_buffer);

        let mut payload_buffer = vec![0u8; length as usize];
        self.stream.read_exact(&mut payload_buffer)?;

        let command: Command = bincode::deserialize(&payload_buffer)?;
        Ok(command)
    }
}

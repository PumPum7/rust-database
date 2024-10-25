mod tests;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::Database;
use database::Value;

pub struct Server {
    db: Arc<Mutex<Database>>,
    port: u16,
}

impl Server {
    pub fn new(db: Database, port: u16) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
            port,
        }
    }

    pub fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))?;
        println!("Server listening on port {}", self.port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let db = Arc::clone(&self.db);
                    std::thread::spawn(move || {
                        if let Err(e) = handle_client(stream, db) {
                            eprintln!("Error handling client: {}", e);
                        }
                    });
                }
                Err(e) => eprintln!("Error accepting connection: {}", e),
            }
        }
        Ok(())
    }
}

fn handle_client(mut stream: TcpStream, db: Arc<Mutex<Database>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }

        let response = match handle_command(&line, &db) {
            Ok(resp) => resp,
            Err(e) => format!("ERROR: {}\n", e),
        };

        stream.write_all(response.as_bytes())?;
        stream.flush()?;
    }
    Ok(())
}

fn parse_value(s: &str) -> Result<Value, Box<dyn std::error::Error>> {
    // Try parsing as different types
    if s == "null" {
        return Ok(Value::Null);
    } else if s == "true" {
        return Ok(Value::Boolean(true));
    } else if s == "false" {
        return Ok(Value::Boolean(false));
    } else if let Ok(i) = s.parse::<i64>() {
        return Ok(Value::Integer(i));
    } else if let Ok(f) = s.parse::<f64>() {
        return Ok(Value::Float(f));
    } else {
        // If not a number, treat as string
        Ok(Value::String(s.to_string()))
    }
}

fn handle_command(
    cmd: &str,
    db: &Arc<Mutex<Database>>,
) -> Result<String, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    if parts.is_empty() {
        return Ok("OK\n".to_string());
    }

    let mut db = db.lock().unwrap();
    match parts[0].to_uppercase().as_str() {
        "GET" => {
            if parts.len() != 2 {
                return Ok("ERROR: Usage: GET <key>\n".to_string());
            }
            let key = parts[1].parse::<i32>()?;
            match db.get(key)? {
                Some(value) => Ok(format!("{:?}\n", value)),
                None => Ok("NULL\n".to_string()),
            }
        }
        "SET" => {
            if parts.len() != 3 {
                return Ok("ERROR: Usage: SET <key> <value>\n".to_string());
            }
            let key = parts[1].parse::<i32>()?;
            let value = parse_value(parts[2])?;
            db.insert(key, value)?;
            Ok("OK\n".to_string())
        }
        "DEL" => {
            if parts.len() != 2 {
                return Ok("ERROR: Usage: DEL <key>\n".to_string());
            }
            let key = parts[1].parse::<i32>()?;
            db.delete(key)?;
            Ok("OK\n".to_string())
        }
        "UPDATE" => {
            if parts.len() != 3 {
                return Ok("ERROR: Usage: UPDATE <key> <value>\n".to_string());
            }
            let key = parts[1].parse::<i32>()?;
            let value = parse_value(parts[2])?;
            db.update(key, value)?;
            Ok("OK\n".to_string())
        }
        "EXIT" => {
            std::process::exit(0);
        }
        _ => Ok("ERROR: Unknown command\n".to_string()),
    }
}

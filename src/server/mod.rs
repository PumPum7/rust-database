use database::protocol::ProtocolError;
use database::{protocol, Database, Value};
use protocol::{connection::Connection, Command, Response};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use threadpool::ThreadPool;

mod parser;
mod tests;

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
        let pool = ThreadPool::new(4);
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))?;
        println!("Server listening on port {}", self.port);

        for stream in listener.incoming() {
            let db = Arc::clone(&self.db);
            if let Ok(stream) = stream {
                pool.execute(move || {
                    if let Err(e) = handle_client(stream, db) {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
        }
        Ok(())
    }
}

fn handle_client(stream: TcpStream, db: Arc<Mutex<Database>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = Connection::new(stream);

    loop {
        let raw_command = match conn.receive_raw_command() {
            Ok(cmd) => cmd,
            Err(ProtocolError::ConnectionClosed) => {
                println!("Client disconnected");
                return Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                eprintln!("Error receiving command: {}", error_msg);
                if let Err(send_err) = conn.send_response(Response::Error(error_msg)) {
                    eprintln!("Error sending error response: {}", send_err);
                }
                continue;
            }
        };

        let command = match parse_raw_command(&raw_command) {
            Ok(cmd) => cmd,
            Err(e) => {
                let error_msg = e.to_string();
                if let Err(send_err) = conn.send_response(Response::Error(error_msg)) {
                    eprintln!("Error sending error response: {}", send_err);
                }
                continue;
            }
        };

        let response = match handle_command(command, &db) {
            Ok(resp) => resp,
            Err(e) => Response::Error(e.to_string()),
        };

        if let Err(e) = conn.send_response(response) {
            eprintln!("Error sending response: {}", e);
            return Err(Box::new(e));
        }
    }
}

fn parse_raw_command(raw_command: &str) -> Result<Command, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = raw_command.trim().split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".into());
    }

    match parts[0].to_uppercase().as_str() {
        "GET" => {
            if parts.len() != 2 {
                return Err("Usage: GET <key>".into());
            }
            Ok(Command::Get { key: parts[1].parse()? })
        },
        "SET" => {
            if parts.len() < 3 {
                return Err("Usage: SET <key> <value>".into());
            }
            Ok(Command::Set {
                key: parts[1].parse()?,
                value: parse_value(&parts[2..].join(" "))?,
            })
        },
        "UPDATE" => {
            if parts.len() < 3 {
                return Err("Usage: UPDATE <key> <value>".into());
            }
            Ok(Command::Update { key: parts[1].parse()?, value: parse_value(&parts[2..].join(" "))? })
        },
        "DEL" => {
            if parts.len() != 2 {
                return Err("Usage: DEL <key>".into());
            }
            Ok(Command::Delete { key: parts[1].parse()? })
        },
        "ALL" => {
            Ok(Command::All)
        },
        "STRLEN" => {
            if parts.len() != 2 {
                return Err("Usage: STRLEN <key>".into());
            }
            Ok(Command::Strlen { key: parts[1].parse()? })
        },
        "STRCAT" => {
            if parts.len() < 3 {
                return Err("Usage: STRCAT <key> <value>".into());
            }
            Ok(Command::Strcat { key: parts[1].parse()?, value: parse_value(&parts[2..].join(" "))? })
        },
        "SUBSTR" => {
            if parts.len() != 4 {
                return Err("Usage: SUBSTR <key> <start> <length>".into());
            }
            Ok(Command::Substr { key: parts[1].parse()?, start: parts[2].parse()?, length: parts[3].parse()? })
        },
        _ => Err("Unknown command".into()),
    }
}

fn parse_value(s: &str) -> Result<Value, Box<dyn std::error::Error>> {
    if s == "null" {
        Ok(Value::Null)
    } else if s == "true" {
        Ok(Value::Boolean(true))
    } else if s == "false" {
        Ok(Value::Boolean(false))
    } else if let Ok(i) = s.parse::<i64>() {
        Ok(Value::Integer(i))
    } else if let Ok(f) = s.parse::<f64>() {
        Ok(Value::Float(f))
    } else {
        Ok(Value::String(s.to_string()))
    }
}

fn handle_command(
    command: Command,
    db: &Arc<Mutex<Database>>,
) -> Result<Response, Box<dyn std::error::Error>> {
    let mut db = db.lock().unwrap();

    match command {
        Command::Get { key } => {
            let value = db.get(key)?;
            Ok(Response::Value(value))
        }
        Command::Set { key, value } => {
            db.insert(key, &value)?;
            Ok(Response::Ok)
        }
        Command::Delete { key } => {
            db.delete(key)?;
            Ok(Response::Ok)
        }
        Command::Update { key, value } => {
            db.update(key, &value)?;
            Ok(Response::Ok)
        }
        Command::All => {
            let results = db.all()?;
            Ok(Response::Range(results))
        }
        Command::Strlen { key } => {
            let size = db.strlen(key)?.unwrap_or(0);
            Ok(Response::Size(size))
        }
        Command::Expression(expr) => {
            let tokens: Vec<&str> = expr.split_whitespace().collect();
            match parser::parse_expression(&tokens, &mut db) {
                Ok(value) => Ok(Response::Value(Some(value))),
                Err(e) => Ok(Response::Error(e.to_string())),
            }
        }
        Command::Ping => Ok(Response::Pong),
        Command::Exit => std::process::exit(0),
        _ => Ok(Response::Error("Unknown command".into())),
    }
}
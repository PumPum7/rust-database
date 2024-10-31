use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard};
use threadpool::ThreadPool;

use crate::command::Command;
use crate::{
    database_handler::database_handler::Database, protocol::connection::Connection, protocol::error::ProtocolError,
    protocol::response::Response,
};

mod parser;

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

fn handle_client(
    stream: TcpStream,
    mut db: Arc<Mutex<Database>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = Connection::new(stream);
    loop {
        let raw_command = match conn.receive_raw_command() {
            Ok(cmd) => cmd.to_string(),
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

        let command = match parser::parse_raw_command(&raw_command, &mut db) {
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

fn handle_command(
    command: Command,
    db: &Arc<Mutex<Database>>,
) -> Result<Response, Box<dyn std::error::Error>> {
    match command {
        Command::Get { key } => {
            let mut db = lock_db(&db)?;
            let value = db.get(key)?;
            Ok(Response::Value(value))
        }
        Command::Set { key, value } => {
            let mut db = lock_db(&db)?;
            db.insert(key, &value)?;
            Ok(Response::Ok)
        }
        Command::Delete { key } => {
            let mut db = lock_db(&db)?;
            db.delete(key)?;
            Ok(Response::Ok)
        }
        Command::Update { key, value } => {
            let mut db = lock_db(&db)?;
            db.update(key, &value)?;
            Ok(Response::Ok)
        }
        Command::All => {
            let mut db = lock_db(&db)?;
            let results = db.all()?;
            Ok(Response::Range(results))
        }
        Command::Strlen { key } => {
            let mut db = lock_db(&db)?;
            let size = db.strlen(key)?.unwrap_or(0);
            Ok(Response::Size(size))
        }
        Command::Expression(expr) => {
            let mut db = db.clone();
            match parser::evaluate_expression(&expr, &mut db) {
                Ok(value) => Ok(Response::Value(Some(value))),
                Err(e) => Ok(Response::Error(e.to_string())),
            }
        }
        Command::Ping => Ok(Response::Pong),
        Command::Exit => std::process::exit(0),
        _ => Ok(Response::Error("Unknown command".into())),
    }
}

fn lock_db(db: &Arc<Mutex<Database>>) -> Result<MutexGuard<Database>, Box<dyn std::error::Error>> {
    Ok(db.lock().unwrap())
}

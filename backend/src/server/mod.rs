use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard};
use threadpool::ThreadPool;

use crate::command::Command;
use crate::database_handler::database_handler::{Database, IsolationLevel};
use crate::protocol::connection::Connection;
use crate::protocol::error::ProtocolError;
use crate::protocol::response::Response;
use crate::storage::TransactionId;

mod parser;

type ClientTxnMap = Arc<Mutex<HashMap<SocketAddr, TransactionId>>>;

pub struct Server {
    db: Arc<Mutex<Database>>,
    port: u16,
    client_txns: ClientTxnMap,
}

impl Server {
    pub fn new(db: Database, port: u16) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
            port,
            client_txns: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn run(&self) -> std::io::Result<()> {
        let pool = ThreadPool::new(4);
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))?;
        println!("Server listening on port {}", self.port);

        for stream in listener.incoming() {
            let db = Arc::clone(&self.db);
            let client_txns = Arc::clone(&self.client_txns);
            if let Ok(stream) = stream {
                pool.execute(move || {
                    if let Err(e) = handle_client(stream, db, client_txns) {
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
    client_txns: ClientTxnMap,
) -> Result<(), Box<dyn std::error::Error>> {
    let peer_addr = stream.peer_addr()?;
    let mut conn = Connection::new(stream);

    loop {
        let raw_command = match conn.receive_raw_command() {
            Ok(cmd) => cmd.to_string(),
            Err(ProtocolError::ConnectionClosed) => {
                println!("Client {} disconnected", peer_addr);
                client_txns.lock().unwrap().remove(&peer_addr);
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

        let response = match handle_command(command, &db, peer_addr, &client_txns) {
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
    peer_addr: SocketAddr,
    client_txns: &ClientTxnMap,
) -> Result<Response, Box<dyn std::error::Error>> {
    match command {
        Command::Begin { isolation } => {
            let isolation_level = match isolation.as_str() {
                "read_committed" | "rc" => IsolationLevel::ReadCommitted,
                "repeatable_read" | "rr" => IsolationLevel::RepeatableRead,
                _ => IsolationLevel::ReadCommitted,
            };
            let mut db = lock_db(&db)?;
            let txn_id = db.begin_transaction(isolation_level)?;
            client_txns.lock().unwrap().insert(peer_addr, txn_id);
            Ok(Response::Value(Some(
                crate::storage::value::Value::Integer(txn_id.0 as i64),
            )))
        }
        Command::Commit { txn_id } => {
            let actual_txn_id = if let Some(id) = txn_id {
                TransactionId(id)
            } else {
                let map = client_txns.lock().unwrap();
                *map.get(&peer_addr).ok_or("No active transaction")?
            };
            let mut db = lock_db(&db)?;
            db.commit_transaction(actual_txn_id)?;
            client_txns.lock().unwrap().remove(&peer_addr);
            Ok(Response::Ok)
        }
        Command::Rollback { txn_id } => {
            let actual_txn_id = if let Some(id) = txn_id {
                TransactionId(id)
            } else {
                let map = client_txns.lock().unwrap();
                *map.get(&peer_addr).ok_or("No active transaction")?
            };
            let mut db = lock_db(&db)?;
            db.rollback_transaction(actual_txn_id)?;
            client_txns.lock().unwrap().remove(&peer_addr);
            Ok(Response::Ok)
        }
        Command::Get { key } => {
            let mut db = lock_db(&db)?;
            let value = db.get(key)?;
            Ok(Response::Value(value))
        }
        Command::Set { key, value, txn_id } => {
            let mut db = lock_db(&db)?;
            if let Some(t_id) = txn_id {
                db.insert_in_transaction(TransactionId(t_id), key, &value)?;
            } else {
                db.insert(key, &value)?;
            }
            Ok(Response::Ok)
        }
        Command::Delete { key, txn_id } => {
            let mut db = lock_db(&db)?;
            if let Some(t_id) = txn_id {
                db.delete_in_transaction(TransactionId(t_id), key)?;
            } else {
                db.delete(key)?;
            }
            Ok(Response::Ok)
        }
        Command::Update { key, value, txn_id } => {
            let mut db = lock_db(&db)?;
            if let Some(t_id) = txn_id {
                db.update_in_transaction(TransactionId(t_id), key, &value)?;
            } else {
                db.update(key, &value)?;
            }
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
        Command::Strcat { key, value, txn_id } => {
            let mut db = lock_db(&db)?;
            if let Some(t_id) = txn_id {
                db.strcat_in_transaction(TransactionId(t_id), key, &value)?;
            } else {
                db.strcat(key, &value)?;
            }
            Ok(Response::Ok)
        }
        Command::Substr {
            key,
            start,
            length,
            txn_id,
        } => {
            let mut db = lock_db(&db)?;
            if let Some(t_id) = txn_id {
                db.substr_in_transaction(TransactionId(t_id), key, start, length)?;
            } else {
                db.substr(key, start, length)?;
            }
            Ok(Response::Ok)
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
    }
}

fn lock_db(db: &Arc<Mutex<Database>>) -> Result<MutexGuard<Database>, Box<dyn std::error::Error>> {
    db.lock().map_err(|e| {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to acquire database lock: {}", e),
        )) as Box<dyn std::error::Error>
    })
}

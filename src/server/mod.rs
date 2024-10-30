use protocol::{Command, Response, connection::Connection};
use database::{protocol, Database};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use threadpool::ThreadPool;

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
    db: Arc<Mutex<Database>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = Connection::new(stream);

    loop {
        let command = match conn.receive_command() {
            Ok(cmd) => cmd,
            Err(e) => {
                conn.send_response(Response::Error(e.to_string()))?;
                continue;
            }
        };

        let response = handle_command(command, &db)?;
        conn.send_response(response)?;
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
        Command::Ping => Ok(Response::Pong),
        Command::Exit => std::process::exit(0),
        _ => Ok(Response::Error("Unknown command".into())),
    }
}

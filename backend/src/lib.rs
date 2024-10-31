pub mod btree;
pub mod command;
pub mod database_handler;
pub mod protocol;
pub mod server;
pub mod storage;

#[cfg(test)]
pub mod tests;

use database_handler::database_handler::Database;
use server::Server;
use std::error::Error;

use log::{error, info};

fn print_header() {
    println!(
        r#"
  ____        _        _
 |  _ \  __ _| |_ __ _| |__   __ _ ___  ___
 | | | |/ _` | __/ _` | '_ \ / _` / __|/ _ \
 | |_| | (_| | || (_| | |_) | (_| \__ \  __/
 |____/ \__,_|\__\__,_|_.__/ \__,_|___/\___|

        "#
    );
}

pub fn run() -> Result<(), Box<dyn Error>> {
    print_header();
    env_logger::init();
    info!("Starting database server");

    // Create or open database
    let db = match Database::new("test.db") {
        Ok(db) => {
            info!("Successfully opened database");
            db
        }
        Err(e) => {
            error!("Failed to open database: {}", e);
            return Err(e);
        }
    };

    // Create and run server
    let server = Server::new(db, 5432);
    info!("Starting server on port 5432");
    match server.run() {
        Ok(_) => info!("Server stopped"),
        Err(e) => error!("Server stopped with error: {}", e),
    };

    Ok(())
}

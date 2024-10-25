use database::Database;
use std::error::Error;

mod server;
use server::Server;

fn main() -> Result<(), Box<dyn Error>> {
    println!("SQLite-like Database Engine Starting...");

    // Create or open database
    let db = match Database::new("test.db") {
        Ok(db) => {
            println!("Successfully opened database");
            db
        }
        Err(e) => {
            eprintln!("Failed to open database: {}", e);
            return Err(e);
        }
    };

    // Create and run server
    let server = Server::new(db, 5432);
    println!("Starting server on port 5432");
    server.run()?;

    Ok(())
}

use database::database::Database;
use server::Server;
use std::error::Error;

mod server;

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

fn main() -> Result<(), Box<dyn Error>> {
    print_header();
    println!("Starting database server");

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

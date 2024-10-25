# Database Project

This project implements a simple database system with a client-server architecture.

## Features

- TCP-based client-server communication
- Interactive command-line interface for the client
- Basic key-value store operations (GET, SET, UPDATE, DEL)
- SQL parsing (in progress)
- B-tree index for efficient data storage and retrieval

## Project Structure

- `src/bin/client.rs`: Implementation of the database client
- `src/query/parser.rs`: SQL parser (to be implemented)
- `src/server/lib.rs`: Server-side logic and storage management
- `src/index/mod.rs`: B-tree index implementation

## Getting Started

1. Clone the repository
2. Run `cargo build` to compile the project
3. Start the server with `cargo run`
4. Run the client with `cargo run --bin client`

## Usage

Once connected to the database, you can use the following commands:

- `GET <key>`: Retrieve the value associated with the given key
- `SET <key> <value>`: Set a key-value pair
- `UPDATE <key> <value>`: Update an existing key-value pair
- `DEL <key>`: Delete a key-value pair
- `EXIT`: Quit the client
- `HELP`: Display available commands

Use TAB for command completion in the client interface.

## Future Improvements

- Implement SQL parsing and query execution
- Add support for more complex data structures and operations
- Improve error handling and logging
- Implement data persistence

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.


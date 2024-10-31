# Database Project

This project implements a simple database system with a client-server architecture.

## Features

- TCP-based client-server communication
- Interactive command-line interface for the client
- Basic key-value store operations (GET, SET, UPDATE, DEL)
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
- `SET <key> <value>`: Set a key-value pair (also supports operations like +, -, ...)
- `UPDATE <key> <value>`: Update an existing key-value pair
- `DEL <key>`: Delete a key-value pair
- `STRLEN <key>`: Get the length of the value associated with the given key
- `STRCAT <key> <key2>`: Concatenate the values of two keys and store the result in a third key
- `SUBSTR <key> <start> <end>`: Get a substring of the value associated with the given key
- `exit`: Quit the client
- `help`: Display available commands

### Expression Examples:
- `EXPR(GET 1 + GET 2)`: Retrieve the value associated with key 1 and key 2, then add them together.
- `EXPR(GET 1 * 2)`: Retrieve the value associated with key 1 and multiply it by 2.

Use TAB for command completion in the client interface.

## Future Improvements

- Improve error handling and logging

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.


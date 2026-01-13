# Database Project

A Rust-based database system with ACID transaction support, MVCC, and a client-server architecture.

## Features

- **ACID Transactions**: Full transaction support with BEGIN, COMMIT, and ROLLBACK
- **Isolation Levels**: Read Committed and Repeatable Read isolation
- **MVCC**: Multi-Version Concurrency Control for snapshot isolation
- **Write-Ahead Logging**: ARIES-style WAL with checkpoints, compensation log records (CLRs), and 3-phase crash recovery (Analysis, Redo, Undo)
- **B-Tree Index**: Efficient key-value storage and retrieval
- **TCP Protocol**: Binary protocol for client-server communication
- **Interactive CLI**: Command-line client with tab completion

## Project Structure

```
.
├── backend/              # Database server implementation
│   └── src/
│       ├── btree/        # B-tree index implementation
│       ├── command/      # Command parsing and types
│       ├── database_handler/  # Database operations
│       ├── protocol/     # Network protocol (frames, connections)
│       ├── server/       # TCP server and request handling
│       ├── storage/      # Storage engine
│       │   ├── buffer_pool.rs   # In-memory page cache
│       │   ├── disk_manager.rs  # Disk I/O
│       │   ├── mvcc.rs          # Multi-version concurrency control
│       │   ├── transaction.rs   # Transaction and snapshot management
│       │   ├── wal.rs           # Write-ahead logging
│       │   └── value.rs         # Value types
│       └── tests/        # Unit tests
└── database-client/      # Interactive CLI client
```

## Getting Started

```bash
# Build the project
cargo build

# Start the server (default port: 5432)
cargo run --bin server

# In another terminal, start the client
cargo run --package database-client
```

## Commands

### Basic Operations

| Command | Description |
|---------|-------------|
| `GET <key>` | Retrieve value by key |
| `SET <key> <value>` | Store a key-value pair |
| `UPDATE <key> <value>` | Update existing value |
| `DEL <key>` | Delete a key-value pair |
| `ALL` | List all key-value pairs |

### String Operations

| Command | Description |
|---------|-------------|
| `STRLEN <key>` | Get string length |
| `STRCAT <key> <value>` | Append to string value |
| `SUBSTR <key> <start> <len>` | Extract substring |

### Transaction Commands

| Command | Description |
|---------|-------------|
| `BEGIN [isolation]` | Start transaction (returns txn_id) |
| `COMMIT` | Commit current transaction |
| `ROLLBACK` | Abort current transaction |

Isolation levels: `read_committed` or `rc` (default), `repeatable_read` or `rr`

### Maintenance Commands

| Command | Description |
|---------|-------------|
| `VACUUM` | Run garbage collection on old versions |
| `PING` | Test server connectivity |

### Transactional Operations

Commands can be executed within a transaction using the `TXN <id>` suffix:

```
BEGIN rr                 # Returns: 1 (RepeatableRead isolation)
SET 1 100 TXN 1          # Insert within transaction
SET 2 200 TXN 1          # Another insert
GET 1 TXN 1              # Read within transaction (sees own writes)
ALL TXN 1                # List all within transaction snapshot
COMMIT                   # Commit all changes
```

Read commands that support transaction context:
- `GET <key> [TXN <id>]`
- `ALL [TXN <id>]`
- `STRLEN <key> [TXN <id>]`

### Expression Evaluation

```
EXPR(GET 1 + GET 2)      # Add values of key 1 and 2
SET 3 EXPR(GET 1 * 2)    # Set key 3 to double of key 1
EXPR(GET 1 + 3.14)       # Mix keys and literals
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Client Layer                            │
│              TCP Connection + Binary Protocol                │
└─────────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────────┐
│                   Transaction Layer                          │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────────────┐  │
│  │ Isolation   │ │   Snapshot   │ │ Transaction Manager  │  │
│  │ RC/RR/Ser   │ │  Visibility  │ │   Active/Committed   │  │
│  └─────────────┘ └──────────────┘ └──────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────────┐
│                      MVCC Layer                              │
│         VersionedValue { value, xmin, xmax }                 │
└─────────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────────┐
│                    Storage Layer                             │
│  B-Tree Index → Buffer Pool → Disk Manager → WAL            │
└─────────────────────────────────────────────────────────────┘
```

## MVCC (Multi-Version Concurrency Control)

The database uses MVCC to provide snapshot isolation:

- **Version Tracking**: Each value stores `xmin` (creating transaction) and `xmax` (deleting transaction)
- **Visibility Rules**: Transactions only see versions created by committed transactions before their snapshot
- **No Read Locks**: Readers never block writers; writers never block readers
- **Garbage Collection**: Use `VACUUM` to clean up old versions no longer visible to any transaction

### Isolation Levels

| Level | Description |
|-------|-------------|
| Read Committed (`rc`) | Each statement sees latest committed data |
| Repeatable Read (`rr`) | Transaction sees consistent snapshot from start |

### Example: Repeatable Read Isolation

```
# Terminal 1                    # Terminal 2
BEGIN rr                        
GET 1                           # Returns: 100
                                UPDATE 1 200
                                COMMIT
GET 1                           # Still returns: 100 (snapshot)
COMMIT
GET 1                           # Now returns: 200
```

## Development

```bash
# Run all tests
cargo test

# Run specific test module
cargo test btree_tests
cargo test transaction_tests
cargo test mvcc_tests

# Run a single test
cargo test test_btree_operations -- --exact

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Value Types

The database supports multiple value types:

- `Integer(i64)` - 64-bit signed integers
- `Float(f64)` - 64-bit floating point
- `String(String)` - UTF-8 strings
- `Boolean(bool)` - true/false
- `Null` - null value

## Configuration

Currently hardcoded defaults (configuration file support planned):

| Setting | Default |
|---------|---------|
| Server port | 5432 |
| Buffer pool size | 1000 pages |
| Page size | 4KB |
| Thread pool size | 4 |

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

See [LICENSE](LICENSE) file.

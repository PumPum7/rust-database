# Database Project Knowledge

## Overview
- Custom database implementation in Rust
- B-tree index structure for efficient key-value storage
- Client-server architecture with TCP communication
- ACID compliant with transaction support

## Key Components
- Storage engine with buffer pool and disk management
- B-tree implementation for indexing
- Slotted page design for record storage
- TCP server for client connections

## Design Decisions
- Page size: 4KB (standard size for most systems)
- B-tree order: 4 (configurable via ORDER constant)
- Buffer pool capacity: 1000 pages

## Style Guidelines
- Use Result type for error handling
- Implement Debug trait for key structs
- Keep functions focused and single-purpose
- Add error context in public APIs

## Testing Strategy
- Unit tests for each component
- Integration tests for database operations
- Separate test files for complex components

## Future Improvements
- Add WAL (Write-Ahead Logging)
- Implement query optimizer
- Add support for secondary indexes
- Improve concurrency with better locking

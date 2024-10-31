use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Page {0} not found")]
    PageNotFound(u32),

    #[error("Buffer pool is full")]
    BufferPoolFull,

    #[error("Invalid page")]
    InvalidPage,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Page is full")]
    PageFull,

    #[error("Invalid slot")]
    InvalidSlot,

    #[error("Invalid record")]
    InvalidRecord,

    #[error("Record was deleted")]
    DeletedRecord,

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Key not found")]
    KeyNotFound(i32),

    #[error("Transaction not active")]
    TransactionNotActive,

    #[error("Transaction already committed")]
    TransactionAlreadyCommitted,
}

pub type Result<T> = std::result::Result<T, DatabaseError>;

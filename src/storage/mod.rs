pub mod buffer_pool;
pub mod disk_manager;
pub mod error;
pub mod page;
pub mod slotted_page;
pub mod transaction;
pub mod value;
pub mod wal;
pub mod operations;

mod tests;
pub use value::Value;
pub use wal::{WriteAheadLog, LogRecord};

pub use transaction::{Transaction, TransactionManager};

// Re-export main components
pub use buffer_pool::BufferPool;
pub use disk_manager::DiskManager;
pub use page::Page;
pub use slotted_page::SlottedPage;

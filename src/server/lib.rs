mod server;
mod storage;

pub use server::Server;
use storage::{Database, Value};

// Re-export commonly used types
pub use storage::{Database, Value};
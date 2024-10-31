use crate::storage::value::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Get {
        key: i32,
    },
    Set {
        key: i32,
        value: Value,
    },
    Delete {
        key: i32,
    },
    Update {
        key: i32,
        value: Value,
    },
    All,
    Strlen {
        key: i32,
    },
    Strcat {
        key: i32,
        value: Value,
    },
    Substr {
        key: i32,
        start: usize,
        length: usize,
    },
    Ping,
    Exit,
    Expression(String),
}

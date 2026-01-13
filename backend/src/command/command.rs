use crate::storage::value::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Begin {
        isolation: String,
    },
    Commit {
        txn_id: Option<u64>,
    },
    Rollback {
        txn_id: Option<u64>,
    },
    Get {
        key: i32,
    },
    Set {
        key: i32,
        value: Value,
        txn_id: Option<u64>,
    },
    Delete {
        key: i32,
        txn_id: Option<u64>,
    },
    Update {
        key: i32,
        value: Value,
        txn_id: Option<u64>,
    },
    All,
    Strlen {
        key: i32,
    },
    Strcat {
        key: i32,
        value: Value,
        txn_id: Option<u64>,
    },
    Substr {
        key: i32,
        start: usize,
        length: usize,
        txn_id: Option<u64>,
    },
    Ping,
    Exit,
    Expression(String),
}

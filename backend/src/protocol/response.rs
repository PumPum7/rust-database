use crate::storage::value::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Value(Option<Value>),
    Range(Vec<(i32, Value)>),
    Error(String),
    Pong,
    Size(usize),
}

use crate::{
    command::Command, database_handler::database_handler::Database, storage::value::Value,
};
use std::sync::{Arc, Mutex};

const VALID_EXPRESSION_METHODS: &str = "-+*/%";

pub fn evaluate_expression(
    expr: &str,
    db: &mut Arc<Mutex<Database>>,
) -> Result<Value, Box<dyn std::error::Error>> {
    for method in VALID_EXPRESSION_METHODS.chars() {
        if expr.contains(method) {
            let parts: Vec<&str> = expr.split(method).collect();
            return if parts.len() == 2 {
                let left = evaluate_expression(parts[0].trim(), db)
                    .map_err(|e| format!("Error evaluating left operand: {}", e))?;
                let right = evaluate_expression(parts[1].trim(), db)
                    .map_err(|e| format!("Error evaluating right operand: {}", e))?;
                handle_expression(&left, &right, method)
            } else {
                Err(format!("Invalid expression format: {}", expr).into())
            };
        }
    }

    if expr.starts_with("GET") {
        let mut db = db
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        let key = expr[4..].trim().parse::<i32>()?;
        return Ok(db
            .get(key)
            .unwrap_or(Some(Value::Integer(0)))
            .expect("Error getting value"));
    } else if expr.starts_with("STRLEN") {
        let mut db = db
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        let key = expr[6..].trim().parse::<i32>()?;
        return Ok(Value::Integer(
            db.get(key)
                .unwrap_or(Some(Value::String("".to_string())))
                .expect("Error getting value")
                .to_string()
                .len() as i64,
        ));
    }

    parse_value(expr)
}

pub fn parse_value(s: &str) -> Result<Value, Box<dyn std::error::Error>> {
    if s == "null" {
        Ok(Value::Null)
    } else if s == "true" {
        Ok(Value::Boolean(true))
    } else if s == "false" {
        Ok(Value::Boolean(false))
    } else if let Ok(i) = s.parse::<i64>() {
        Ok(Value::Integer(i))
    } else if let Ok(f) = s.parse::<f64>() {
        Ok(Value::Float(f))
    } else {
        Ok(Value::String(s.to_string()))
    }
}

pub fn parse_raw_command(
    raw_command: &str,
    db: &mut Arc<Mutex<Database>>,
) -> Result<Command, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = raw_command.trim().split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".into());
    }

    let get_txn_id = |parts: &[&str]| -> Option<u64> {
        parts
            .iter()
            .position(|p| p.to_uppercase() == "TXN")
            .and_then(|idx| parts.get(idx + 1).and_then(|id| id.parse().ok()))
    };

    match parts[0].to_uppercase().as_str() {
        "BEGIN" => {
            let isolation = parts.get(1).unwrap_or(&"read_committed").to_lowercase();
            Ok(Command::Begin { isolation })
        }
        "COMMIT" => {
            let txn_id = get_txn_id(&parts);
            Ok(Command::Commit { txn_id })
        }
        "ROLLBACK" => {
            let txn_id = get_txn_id(&parts);
            Ok(Command::Rollback { txn_id })
        }
        "GET" => {
            if parts.len() < 2 {
                return Err("Usage: GET <key> [TXN <id>]".into());
            }
            let txn_id = get_txn_id(&parts);
            Ok(Command::Get {
                key: parts[1].parse()?,
                txn_id,
            })
        }
        "SET" => {
            if parts.len() < 3 {
                return Err("Usage: SET <key> <value> [TXN <id>]".into());
            }

            let txn_id = get_txn_id(&parts);
            let value_part = if txn_id.is_some() {
                let idx = parts
                    .iter()
                    .position(|p| p.to_uppercase() == "TXN")
                    .unwrap();
                parts[2..idx].join(" ")
            } else {
                parts[2..].join(" ")
            };

            if value_part.starts_with("EXPR(") && value_part.ends_with(")") {
                let expr = value_part[5..value_part.len() - 1].trim();
                let result = evaluate_expression(expr, db)?;

                return Ok(Command::Set {
                    key: parts[1].parse()?,
                    value: result,
                    txn_id,
                });
            }
            Ok(Command::Set {
                key: parts[1].parse()?,
                value: parse_value(&value_part)?,
                txn_id,
            })
        }
        "EXPR" => {
            if !raw_command.starts_with("EXPR(") || !raw_command.ends_with(")") {
                return Err("Expression must be in format EXPR(<expression>)".into());
            }
            let expr = raw_command[5..raw_command.len() - 1].trim();
            Ok(Command::Expression(expr.to_string()))
        }
        "UPDATE" => {
            if parts.len() < 3 {
                return Err("Usage: UPDATE <key> <value> [TXN <id>]".into());
            }

            let txn_id = get_txn_id(&parts);
            let value_part = if txn_id.is_some() {
                let idx = parts
                    .iter()
                    .position(|p| p.to_uppercase() == "TXN")
                    .unwrap();
                parts[2..idx].join(" ")
            } else {
                parts[2..].join(" ")
            };

            if value_part.starts_with("EXPR(") && value_part.ends_with(")") {
                let expr = value_part[5..value_part.len() - 1].trim();
                let result = evaluate_expression(expr, db)?;
                return Ok(Command::Update {
                    key: parts[1].parse()?,
                    value: result,
                    txn_id,
                });
            }

            Ok(Command::Update {
                key: parts[1].parse()?,
                value: parse_value(&value_part)?,
                txn_id,
            })
        }
        "DEL" => {
            if parts.len() != 2 {
                return Err("Usage: DEL <key> [TXN <id>]".into());
            }
            let txn_id = get_txn_id(&parts);
            Ok(Command::Delete {
                key: parts[1].parse()?,
                txn_id,
            })
        }
        "ALL" => {
            let txn_id = get_txn_id(&parts);
            Ok(Command::All { txn_id })
        }
        "STRLEN" => {
            if parts.len() < 2 {
                return Err("Usage: STRLEN <key> [TXN <id>]".into());
            }
            let txn_id = get_txn_id(&parts);
            Ok(Command::Strlen {
                key: parts[1].parse()?,
                txn_id,
            })
        }
        "VACUUM" => Ok(Command::Vacuum),
        "STRCAT" => {
            if parts.len() < 3 {
                return Err("Usage: STRCAT <key> <value> [TXN <id>]".into());
            }

            let txn_id = get_txn_id(&parts);
            let value_part = if txn_id.is_some() {
                let idx = parts
                    .iter()
                    .position(|p| p.to_uppercase() == "TXN")
                    .unwrap();
                parts[2..idx].join(" ")
            } else {
                parts[2..].join(" ")
            };

            Ok(Command::Strcat {
                key: parts[1].parse()?,
                value: parse_value(&value_part)?,
                txn_id,
            })
        }
        "SUBSTR" => {
            if parts.len() != 4 {
                return Err("Usage: SUBSTR <key> <start> <length> [TXN <id>]".into());
            }

            let txn_id = get_txn_id(&parts);
            Ok(Command::Substr {
                key: parts[1].parse()?,
                start: parts[2].parse()?,
                length: parts[3].parse()?,
                txn_id,
            })
        }
        _ => Err("Unknown command".into()),
    }
}

fn handle_expression(
    left: &Value,
    right: &Value,
    method: char,
) -> Result<Value, Box<dyn std::error::Error>> {
    match method {
        '+' => Ok(left.add(right)?),
        '-' => Ok(left.sub(right)?),
        '*' => Ok(left.mul(right)?),
        '/' => Ok(left.div(right)?),
        _ => Err("Unknown operator".into()),
    }
}

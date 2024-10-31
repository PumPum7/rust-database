use std::sync::{Arc, Mutex};
use crate::command::Command;
use crate::database_handler::database_handler::Database;
use crate::storage::value::Value;

const VALID_EXPRESSION_METHODS: &str = "-+*/%";

pub fn evaluate_expression(
    expr: &str,
    db: &mut Arc<Mutex<Database>>,
) -> Result<Value, Box<dyn std::error::Error>> {
    // check if expr contains any of the valid expression methods in the string

    for method in VALID_EXPRESSION_METHODS.chars() {
        if expr.contains(method) {
            let parts: Vec<&str> = expr.split(method).collect();
            if parts.len() == 2 {
                let left = evaluate_expression(parts[0].trim(), db)
                    .map_err(|e| format!("Error evaluating left operand: {}", e))?;
                let right = evaluate_expression(parts[1].trim(), db)
                    .map_err(|e| format!("Error evaluating right operand: {}", e))?;
                return handle_expression(&left, &right, method);
            }
        }
    }

    // Check if the expression is a command (GET, STRLEN) are valid commands, if they are, return the value
    if expr.starts_with("GET") {
        let mut db = db.lock().unwrap();
        let key = expr[4..].trim().parse::<i32>()?;
        return Ok(db
            .get(key)
            .unwrap_or(Some(Value::Integer(0)))
            .expect("Error getting value"));
    } else if expr.starts_with("STRLEN") {
        let mut db = db.lock().unwrap();
        let key = expr[6..].trim().parse::<i32>()?;
        return Ok(Value::Integer(
            db.get(key)
                .unwrap_or(Some(Value::String("".to_string())))
                .expect("Error getting value")
                .to_string()
                .len() as i64,
        ));
    }

    // if no valid expression methods are found, parse the expression as a literal
    if let Ok(i) = expr.parse::<i64>() {
        Ok(Value::Integer(i))
    } else if let Ok(f) = expr.parse::<f64>() {
        Ok(Value::Float(f))
    } else if expr == "true" {
        Ok(Value::Boolean(true))
    } else if expr == "false" {
        Ok(Value::Boolean(false))
    } else if expr == "null" {
        Ok(Value::Null)
    } else {
        Ok(Value::String(expr.to_string()))
    }
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

    match parts[0].to_uppercase().as_str() {
        "GET" => {
            if parts.len() != 2 {
                return Err("Usage: GET <key>".into());
            }
            Ok(Command::Get {
                key: parts[1].parse()?,
            })
        }
        "SET" => {
            if parts.len() < 3 {
                return Err("Usage: SET <key> <value>".into());
            }

            let value_part = parts[2..].join(" ");
            if value_part.starts_with("EXPR(") && value_part.ends_with(")") {
                let expr = value_part[5..value_part.len() - 1].trim();
                let result = evaluate_expression(expr, db)?;

                return Ok(Command::Set {
                    key: parts[1].parse()?,
                    value: result,
                });
            }
            Ok(Command::Set {
                key: parts[1].parse()?,
                value: parse_value(&value_part)?,
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
                return Err("Usage: UPDATE <key> <value>".into());
            }
            let value_part = parts[2..].join(" ");
            if value_part.starts_with("EXPR(") && value_part.ends_with(")") {
                let expr = value_part[5..value_part.len() - 1].trim();
                let result = evaluate_expression(expr, db)?;
                return Ok(Command::Update {
                    key: parts[1].parse()?,
                    value: result,
                });
            }

            Ok(Command::Update {
                key: parts[1].parse()?,
                value: parse_value(&parts[2..].join(" "))?,
            })
        }
        "DEL" => {
            if parts.len() != 2 {
                return Err("Usage: DEL <key>".into());
            }
            Ok(Command::Delete {
                key: parts[1].parse()?,
            })
        }
        "ALL" => Ok(Command::All),
        "STRLEN" => {
            if parts.len() != 2 {
                return Err("Usage: STRLEN <key>".into());
            }
            Ok(Command::Strlen {
                key: parts[1].parse()?,
            })
        }
        "STRCAT" => {
            if parts.len() < 3 {
                return Err("Usage: STRCAT <key> <value>".into());
            }
            Ok(Command::Strcat {
                key: parts[1].parse()?,
                value: parse_value(&parts[2..].join(" "))?,
            })
        }
        "SUBSTR" => {
            if parts.len() != 4 {
                return Err("Usage: SUBSTR <key> <start> <length>".into());
            }
            Ok(Command::Substr {
                key: parts[1].parse()?,
                start: parts[2].parse()?,
                length: parts[3].parse()?,
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

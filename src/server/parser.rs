use crate::{server::Value, Database};
use std::sync::MutexGuard;

pub fn parse_expression(
    tokens: &[&str],
    db: &mut MutexGuard<'_, Database>,
) -> Result<Value, Box<dyn std::error::Error>> {
    if tokens.is_empty() {
        return Err("Empty expression".into());
    }

    match tokens[0] {
        "GET" => {
            if tokens.len() != 2 {
                return Err("Invalid GET syntax".into());
            }
            let key = tokens[1].parse::<i32>()?;
            let value = db.get(key)?;
            match value {
                Some(value) => Ok(value),
                None => Ok(Value::Null),
            }
        }
        "STRLEN" => {
            if tokens.len() != 2 {
                return Err("Invalid STRLEN syntax".into());
            }
            let key = tokens[1].parse::<i32>()?;
            let value = db.strlen(key)?;
            match value {
                Some(value) => Ok(Value::Integer(value as i64)),
                None => Ok(Value::Null),
            }
        }
        "(" => {
            if tokens.len() < 4 || tokens[tokens.len() - 1] != ")" {
                return Err("Invalid parentheses".into());
            }
            let inner_tokens = &tokens[1..tokens.len() - 1];
            parse_operation(inner_tokens, db)
        }
        _ => {
            if tokens.len() == 1 {
                parse_literal(tokens[0])
            } else {
                parse_operation(tokens, db)
            }
        }
    }
}

fn parse_operation(
    tokens: &[&str],
    db: &mut MutexGuard<'_, Database>,
) -> Result<Value, Box<dyn std::error::Error>> {
    if tokens.len() < 3 {
        return Err("Invalid operation syntax".into());
    }

    let operator_index = parse_operator(&tokens[1..].join(" "))?;
    let left_operand = parse_operand(&tokens[0..operator_index].join(" "), db)?;
    let right_operand = parse_operand(&tokens[operator_index + 1..].join(" "), db)?;

    match tokens[operator_index] {
        "+" => Ok(left_operand.add(&right_operand)?),
        "-" => Ok(left_operand.sub(&right_operand)?),
        "*" => Ok(left_operand.mul(&right_operand)?),
        "/" => Ok(left_operand.div(&right_operand)?),
        "=" => Ok(left_operand.eq(&right_operand)?),
        _ => Err("Unknown operator".into()),
    }
}

fn parse_operator(tokens: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let index = tokens
        .chars()
        .position(|c| c == '+' || c == '-' || c == '*' || c == '/' || c == '=')
        .ok_or("Operator not found")?;
    Ok(index)
}

fn parse_operand(
    tokens: &str,
    db: &mut MutexGuard<'_, Database>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let relevant_tokens = tokens.trim_matches(|c| c == '(' || c == ')');

    // Handle GET
    if relevant_tokens.starts_with("GET") {
        println!("Getting value for key: {:?}", relevant_tokens);
        let key = relevant_tokens[4..].parse::<i32>()?;
        println!("Key: {:?}", key);
        let value = db.get(key)?;
        println!("Value: {:?}", value);
        match value {
            Some(value) => Ok(value),
            None => Ok(Value::Null),
        }
    } else {
        parse_literal(relevant_tokens)
    }
}

fn parse_literal(token: &str) -> Result<Value, Box<dyn std::error::Error>> {
    // remove brackets
    if let Ok(i) = token.parse::<i64>() {
        Ok(Value::Integer(i))
    } else if let Ok(f) = token.parse::<f64>() {
        Ok(Value::Float(f))
    } else if token == "true" {
        Ok(Value::Boolean(true))
    } else if token == "false" {
        Ok(Value::Boolean(false))
    } else if token == "null" {
        Ok(Value::Null)
    } else {
        Ok(Value::String(token.to_string()))
    }
}

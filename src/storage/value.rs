#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
}

impl Value {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        match self {
            Value::Integer(i) => {
                buffer.push(1); // type tag
                buffer.extend_from_slice(&i.to_le_bytes());
            }
            Value::Float(f) => {
                buffer.push(2);
                buffer.extend_from_slice(&f.to_le_bytes());
            }
            Value::String(s) => {
                buffer.push(3);
                let bytes = s.as_bytes();
                buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                buffer.extend_from_slice(bytes);
            }
            Value::Boolean(b) => {
                buffer.push(4);
                buffer.push(if *b { 1 } else { 0 });
            }
            Value::Null => {
                buffer.push(0);
            }
        }
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> Result<(Self, usize), Box<dyn std::error::Error>> {
        if buffer.is_empty() {
            return Err("Empty buffer".into());
        }

        match buffer[0] {
            0 => Ok((Value::Null, 1)),
            1 => {
                let value = i64::from_le_bytes(buffer[1..9].try_into()?);
                Ok((Value::Integer(value), 9))
            }
            2 => {
                let value = f64::from_le_bytes(buffer[1..9].try_into()?);
                Ok((Value::Float(value), 9))
            }
            3 => {
                let len = u32::from_le_bytes(buffer[1..5].try_into()?) as usize;
                let s = String::from_utf8(buffer[5..5 + len].to_vec())?;
                Ok((Value::String(s), 5 + len))
            }
            4 => {
                let value = buffer[1] != 0;
                Ok((Value::Boolean(value), 2))
            }
            _ => Err("Invalid type tag".into()),
        }
    }

    pub fn add(&self, other: &Self) -> Result<Self, Box<dyn std::error::Error>> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a + *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(a.clone() + &b)),
            (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a && *b)),
            _ => Err("Invalid types for addition".into()),
        }
    }

    pub fn sub(&self, other: &Self) -> Result<Self, Box<dyn std::error::Error>> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a - *b as f64)),
            (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a || *b)),
            _ => Err("Invalid types for subtraction".into()),
        }
    }

    pub fn eq(&self, other: &Self) -> Result<Value, Box<dyn std::error::Error>> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a == b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a == b)),
            (Value::String(a), Value::String(b)) => Ok(Value::Boolean(a == b)),
            (Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(a == b)),
            (Value::Null, Value::Null) => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn mul(&self, other: &Self) -> Result<Self, Box<dyn std::error::Error>> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a * *b as f64)),
            _ => Err("Invalid types for multiplication".into()),
        }
    }

    pub fn div(&self, other: &Self) -> Result<Self, Box<dyn std::error::Error>> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a / b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a / *b as f64)),
            _ => Err("Invalid types for division".into()),
        }
    }
}

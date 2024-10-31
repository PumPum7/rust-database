#[cfg(test)]
mod tests {
    use crate::storage::value::Value;

    #[test]
    fn test_value_operations() -> Result<(), Box<dyn std::error::Error>> {
        let a = Value::Integer(42);
        let b = Value::Float(3.14);

        assert_eq!(a.add(&b)?, Value::Float(45.14));
        assert_eq!(a.mul(&Value::Integer(2))?, Value::Integer(84));

        Ok(())
    }
}

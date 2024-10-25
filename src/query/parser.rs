pub struct Parser;

impl Parser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, sql: &str) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Implement SQL parsing
        Ok(())
    }
}

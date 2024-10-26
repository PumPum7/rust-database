#[cfg(test)]
mod tests {
    use crate::server::{parse_value, Server};
    use crate::Database;
    use database::storage::Value;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    fn setup_test_server() -> u16 {
        let db = Database::new("test_server.db").unwrap();
        let port = 5433;
        let server = Server::new(db, port);

        thread::spawn(move || {
            server.run().unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        port
    }

    fn send_command(stream: &mut TcpStream, command: &str) -> String {
        writeln!(stream, "{}", command).unwrap();
        stream.flush().unwrap();
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();
        response
    }

    #[test]
    fn test_basic_operations() {
        let port = setup_test_server();
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();

        // Test different value types
        assert_eq!(send_command(&mut stream, "SET 1 42"), "OK\n");
        assert_eq!(send_command(&mut stream, "GET 1"), "Integer(42)\n");

        assert_eq!(send_command(&mut stream, "SET 2 3.14"), "OK\n");
        assert_eq!(send_command(&mut stream, "GET 2"), "Float(3.14)\n");

        assert_eq!(send_command(&mut stream, "SET 3 hello"), "OK\n");
        assert_eq!(send_command(&mut stream, "GET 3"), "String(\"hello\")\n");

        assert_eq!(send_command(&mut stream, "SET 4 true"), "OK\n");
        assert_eq!(send_command(&mut stream, "GET 4"), "Boolean(true)\n");

        assert_eq!(send_command(&mut stream, "SET 5 null"), "OK\n");
        assert_eq!(send_command(&mut stream, "GET 5"), "Null\n");

        // Test deletion
        assert_eq!(send_command(&mut stream, "DEL 1"), "OK\n");
        assert_eq!(send_command(&mut stream, "GET 1"), "NULL\n");

        // Cleanup
        std::fs::remove_file("test_server.db").unwrap();
    }

    #[test]
    fn test_value_parsing() {
        assert_eq!(parse_value("42").unwrap(), Value::Integer(42));
    }
}

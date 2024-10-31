#[cfg(test)]
mod tests {
    use database::protocol::connection::Connection;
    use database::{storage::Value, Database};
    
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    use crate::server::Server;

    fn setup_test_server(test_type: &str) -> u16 {
        let db = Database::new(test_type).unwrap();
        let port = 5433;
        let server = Server::new(db, port);

        thread::spawn(move || {
            server.run().unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        port
    }

    fn send_raw_command(stream: &TcpStream, command: &str) -> String {
        let mut conn = Connection::new(stream.try_clone().unwrap());
        conn.send_raw_command(command).unwrap();
        format!("{:?}\n", conn.receive_response().unwrap()).replace("Value(Some(", "").replace("))", "")
    }

    #[test]
    fn test_basic_operations() {
        let port = setup_test_server("test_basic_operations.db");
        let stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();

        assert_eq!(send_raw_command(&stream, "SET 1 42"), "Ok\n");
        assert_eq!(send_raw_command(&stream, "GET 1"), "Integer(42)\n");

        assert_eq!(send_raw_command(&stream, "SET 2 3.14"), "Ok\n");
        assert_eq!(send_raw_command(&stream, "GET 2"), "Float(3.14)\n");

        assert_eq!(send_raw_command(&stream, "SET 3 hello"), "Ok\n");
        assert_eq!(send_raw_command(&stream, "GET 3"), "String(\"hello\")\n");

        assert_eq!(send_raw_command(&stream, "SET 4 true"), "Ok\n");
        assert_eq!(send_raw_command(&stream, "GET 4"), "Boolean(true)\n");

        assert_eq!(send_raw_command(&stream, "SET 5 null"), "Ok\n");
        assert_eq!(send_raw_command(&stream, "GET 5"), "Null\n", "Testing null value");

        assert_eq!(send_raw_command(&stream, "DEL 1"), "Ok\n");
        assert_eq!(send_raw_command(&stream, "GET 1"), "Value(None)\n", "Testing deletion");

        // Cleanup
        std::fs::remove_file("test_basic_operations.db").unwrap();
    }

    #[test]
    fn test_value_operations() {
        let a = Value::Integer(42);
        let b = Value::Float(3.14);
        assert_eq!(a.add(&b).unwrap(), Value::Float(45.14));
    }
}

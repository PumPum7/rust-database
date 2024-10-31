#[cfg(test)]
mod tests {
    use crate::protocol::connection::Connection;
    use crate::protocol::response::Response;
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    fn setup_test_client() -> TcpStream {
        thread::sleep(Duration::from_millis(100));
        TcpStream::connect("127.0.0.1:5432").unwrap()
    }

    #[test]
    fn test_client_connection() {
        let stream = setup_test_client();
        let mut conn = Connection::new(stream);

        conn.send_raw_command("PING").unwrap();
        let response = conn.receive_response().unwrap();
        assert!(matches!(response, Response::Pong));
    }
}

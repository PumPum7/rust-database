#[cfg(test)]
mod tests {
    use crate::protocol::connection::Connection;
    use crate::protocol::{Command, Response};
    use crate::Value;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn test_protocol_communication() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = Connection::new(stream);

            let command = conn.receive_command().unwrap();
            assert!(matches!(command, Command::Ping));

            conn.send_response(Response::Pong).unwrap();
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut conn = Connection::new(stream);

        conn.send_command(Command::Ping).unwrap();
        let response = conn.receive_response().unwrap();
        assert!(matches!(response, Response::Pong));

        handle.join().unwrap();
    }

    #[test]
    fn test_protocol_update() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let _ = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = Connection::new(stream);

            let command = conn.receive_command().unwrap();
            assert!(matches!(
                command,
                Command::Update {
                    key: 1,
                    value: Value::String(_)
                }
            ));

            conn.send_response(Response::Ok).unwrap();
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut conn = Connection::new(stream);

        conn.send_command(Command::Update {
            key: 1,
            value: Value::String("Hello, world!".to_string()),
        })
        .unwrap();
        let response = conn.receive_response().unwrap();
        assert!(matches!(response, Response::Ok));
    }
}

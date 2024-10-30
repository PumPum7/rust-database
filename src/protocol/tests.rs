#[cfg(test)]
mod tests {
    use crate::protocol::connection::Connection;
    use crate::protocol::Response;

    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn test_raw_command() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = Connection::new(stream);
            
            let cmd = conn.receive_raw_command().unwrap();
            assert_eq!(cmd, "SET 1 GET 2 + 3");
            
            conn.send_response(Response::Ok).unwrap();
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut conn = Connection::new(stream);
        
        conn.send_raw_command("SET 1 GET 2 + 3").unwrap();
        let response = conn.receive_response().unwrap();
        assert!(matches!(response, Response::Ok));

        handle.join().unwrap();
    }
}
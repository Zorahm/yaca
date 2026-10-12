use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

const PORT: u16 = 47831;

/// Kills the server when the test ends, whatever happened in between.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn connect() -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match TcpStream::connect(("127.0.0.1", PORT)) {
            Ok(stream) => return stream,
            Err(_) if Instant::now() < deadline => sleep(Duration::from_millis(50)),
            Err(e) => panic!("the server did not come up: {e}"),
        }
    }
}

#[test]
fn ping_gets_pong_and_a_newline() {
    let _server = Server(Command::new(env!("CARGO_BIN_EXE_pong")).spawn().unwrap());
    let mut stream = connect();
    stream.write_all(b"ping\n").unwrap();
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).unwrap();
    assert_eq!(reply, "pong\n");
}

//! A line server on 127.0.0.1:47831. Each connection gets one line and one reply.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

const PORT: u16 = 47831;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", PORT))?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut line = String::new();
        BufReader::new(stream.try_clone()?).read_line(&mut line)?;
        stream.write_all(b"PONG")?;
    }
    Ok(())
}

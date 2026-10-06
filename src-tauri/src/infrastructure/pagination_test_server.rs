//! Local-only HTTP fixture for exercising the installed Resend SDK and DTO adapters.
use resend_rs::{ConfigBuilder, Resend};
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

/// Accepted sockets may inherit O_NONBLOCK on BSD/macOS. Always configure the
/// stream explicitly, independently of the non-blocking accept loop.
fn read_request_headers(stream: &mut TcpStream, timeout: Duration) -> io::Result<Vec<u8>> {
    stream.set_nonblocking(false)?;
    stream.set_write_timeout(Some(timeout))?;
    let deadline = Instant::now() + timeout;
    let mut request = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out reading request headers",
            ));
        }
        stream.set_read_timeout(Some(remaining))?;
        let size = match stream.read(&mut buffer) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed before request headers",
                ))
            }
            Ok(size) => size,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        request.extend_from_slice(&buffer[..size]);
        if request.len() > 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request headers exceed fixture limit",
            ));
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(request);
        }
    }
}

pub fn mock_history(path: &'static str) -> (Resend, thread::JoinHandle<()>, Vec<String>) {
    let ids: Vec<String> = (1..=321)
        .map(|number| format!("00000000-0000-4000-8000-{number:012}"))
        .collect();
    let data: Vec<Value> = ids.iter().enumerate().map(|(index, id)| json!({
        "id": id, "from": "Support <support@a.example>", "to": ["one@a.example", "two@b.example"],
        "received_for": ["one@a.example", "two@b.example"],
        "subject": "Fixture email", "created_at": if index % 2 == 0 {
            "2026-01-01T00:00:00Z"
        } else {
            "2026-01-01 00:00:00.000000+00"
        },
        "message_id": "<fixture@example.com>", "last_event": "delivered",
        "html": null, "text": null, "bcc": [], "cc": [], "reply_to": [],
    })).collect();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resend = Resend::with_config(
        ConfigBuilder::new("local-fixture-not-a-real-key")
            .base_url(base.parse().unwrap())
            .client(client)
            .build(),
    );
    let handle = thread::spawn(move || {
        // Four independent 100-item remote pages, not offset-sized downloads.
        for start in [0, 100, 200, 300] {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "timed out waiting for cursor page request"
                        );
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("test server failed: {error}"),
                }
            };
            let request = read_request_headers(&mut stream, Duration::from_secs(5))
                .expect("fixture could not read complete request headers");
            let header = String::from_utf8(request).unwrap();
            let target = header
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap();
            let url = reqwest::Url::parse(&format!("http://localhost{target}")).unwrap();
            assert_eq!(url.path(), path);
            let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            assert_eq!(query.get("limit").map(String::as_str), Some("100"));
            assert!(!query.contains_key("offset"));
            assert!(!query.contains_key("before"));
            let expected_after = if start == 0 {
                None
            } else {
                data[start - 1]["id"].as_str()
            };
            assert_eq!(query.get("after").map(String::as_str), expected_after);
            let end = (start + 100).min(data.len());
            let body = json!({ "object": "list", "has_more": end < data.len(), "data": &data[start..end] }).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
    });
    (resend, handle, ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn socket_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        // Force BSD's inherited mode even when these tests run on Linux.
        server.set_nonblocking(true).unwrap();
        (client, server)
    }

    #[test]
    fn nonblocking_accepted_socket_waits_for_delayed_fragmented_headers() {
        let (mut client, mut server) = socket_pair();
        let (started, waiting) = mpsc::channel();
        let handle = thread::spawn(move || {
            started.send(()).unwrap();
            read_request_headers(&mut server, Duration::from_secs(2)).unwrap()
        });
        waiting.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        client
            .write_all(b"GET /emails?limit=100 HTTP/1.1\r\n")
            .unwrap();
        thread::sleep(Duration::from_millis(40));
        client.write_all(b"Host: localhost\r\n\r\n").unwrap();
        assert_eq!(
            handle.join().unwrap(),
            b"GET /emails?limit=100 HTTP/1.1\r\nHost: localhost\r\n\r\n"
        );
    }

    #[test]
    fn incomplete_headers_fail_when_client_disconnects() {
        let (mut client, mut server) = socket_pair();
        client.write_all(b"GET /emails HTTP/1.1\r\n").unwrap();
        drop(client);
        assert_eq!(
            read_request_headers(&mut server, Duration::from_secs(2))
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn idle_clients_still_have_a_bounded_read_deadline() {
        let (_client, mut server) = socket_pair();
        let error = read_request_headers(&mut server, Duration::from_millis(50)).unwrap_err();
        // OSes report SO_RCVTIMEO either as EAGAIN/WouldBlock or TimedOut.
        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
    }
}

//! Local-only HTTP fixture for exercising the installed Resend SDK and DTO adapters.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};
use resend_rs::{ConfigBuilder, Resend};
use serde_json::{json, Value};

pub fn mock_history(path: &'static str) -> (Resend, thread::JoinHandle<()>, Vec<String>) {
    let ids: Vec<String> = (1..=321).map(|number| format!("00000000-0000-4000-8000-{number:012}")).collect();
    let data: Vec<Value> = ids.iter().map(|id| json!({
        "id": id, "from": "Support <support@a.example>", "to": ["one@a.example", "two@b.example"],
        "received_for": ["one@a.example", "two@b.example"],
        "subject": "Fixture email", "created_at": "2026-01-01T00:00:00Z",
        "message_id": "<fixture@example.com>", "last_event": "delivered",
        "html": null, "text": null, "bcc": [], "cc": [], "reply_to": [],
    })).collect();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap();
    let resend = Resend::with_config(ConfigBuilder::new("local-fixture-not-a-real-key")
        .base_url(base.parse().unwrap()).client(client).build());
    let handle = thread::spawn(move || {
        // Four independent 100-item remote pages, not offset-sized downloads.
        for start in [0, 100, 200, 300] {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "timed out waiting for cursor page request");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("test server failed: {error}"),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let size = stream.read(&mut buffer).unwrap();
                assert!(size > 0);
                request.extend_from_slice(&buffer[..size]);
            }
            let header = String::from_utf8(request).unwrap();
            let target = header.lines().next().unwrap().split_whitespace().nth(1).unwrap();
            let url = reqwest::Url::parse(&format!("http://localhost{target}")).unwrap();
            assert_eq!(url.path(), path);
            let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            assert_eq!(query.get("limit").map(String::as_str), Some("100"));
            assert!(!query.contains_key("offset"));
            assert!(!query.contains_key("before"));
            let expected_after = if start == 0 { None } else { data[start - 1]["id"].as_str() };
            assert_eq!(query.get("after").map(String::as_str), expected_after);
            let end = (start + 100).min(data.len());
            let body = json!({ "object": "list", "has_more": end < data.len(), "data": &data[start..end] }).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
    });
    (resend, handle, ids)
}

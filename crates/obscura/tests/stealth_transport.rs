#![cfg(feature = "stealth")]

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::Duration;

use obscura::Browser;
use obscura_net::BROWSER_USER_AGENT;

fn spawn_server() -> (String, mpsc::Receiver<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (request_tx, request_rx) = mpsc::sync_channel(1);

    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
        }
        request_tx
            .send(String::from_utf8(request).unwrap())
            .unwrap();

        let body = "<!doctype html><html><body>ok</body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body,
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    (format!("http://{}", addr), request_rx)
}

async fn navigate_request(stealth: bool) -> String {
    let (url, request_rx) = spawn_server();

    let browser = Browser::builder().stealth(stealth).build().unwrap();
    let mut page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();

    let request = request_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    request
}

fn header(request: &str, name: &str) -> Option<String> {
    request
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find_map(|(header_name, value)| {
            header_name
                .eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
}

#[tokio::test(flavor = "current_thread")]
async fn stealth_transport_requires_compile_time_and_runtime_opt_in() {
    std::env::set_var("OBSCURA_ALLOW_PRIVATE_NETWORK", "1");
    let stealth_request = navigate_request(true).await;
    let ordinary_request = navigate_request(false).await;

    // The identity is deliberately identical in both modes. The transport
    // assertion must therefore use a wire-level property owned by wreq's
    // Chrome emulation, not the UA string.
    assert_eq!(header(&stealth_request, "user-agent").as_deref(), Some(BROWSER_USER_AGENT));
    assert_eq!(header(&ordinary_request, "user-agent").as_deref(), Some(BROWSER_USER_AGENT));
    assert_eq!(header(&stealth_request, "priority").as_deref(), Some("u=0, i"));
    assert!(header(&ordinary_request, "priority").is_none());
}

use std::io::{Read, Write};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use obscura_browser::{BrowserContext, Page};
use obscura_net::{BROWSER_NAVIGATOR_PLATFORM, BROWSER_UA_PLATFORM, BROWSER_USER_AGENT};
use serde_json::json;

fn spawn_identity_server() -> (String, mpsc::Receiver<(String, String)>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = mpsc::sync_channel(2);

    std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
            }

            let headers = String::from_utf8(request).unwrap();
            let path = headers.split_whitespace().nth(1).unwrap_or("/");
            let user_agent = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("user-agent")
                        .then(|| value.trim().to_owned())
                })
                .unwrap_or_default();
            request_tx.send((path.to_owned(), user_agent)).unwrap();

            let body = if path == "/" {
                "<!doctype html><html><body><iframe src=\"/frame.html\"></iframe></body></html>"
            } else {
                "<!doctype html><html><body>frame</body></html>"
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });

    (format!("http://{address}"), request_rx)
}

fn identity_value() -> serde_json::Value {
    json!({
        "ua": BROWSER_USER_AGENT,
        "platform": BROWSER_NAVIGATOR_PLATFORM,
        "uaPlatform": BROWSER_UA_PLATFORM,
    })
}

fn identity_expression() -> &'static str {
    "({ua: navigator.userAgent, platform: navigator.platform, uaPlatform: navigator.userAgentData.platform})"
}

#[tokio::test(flavor = "current_thread")]
async fn default_identity_agrees_across_http_page_iframe_and_worker() {
    std::env::set_var("OBSCURA_ALLOW_PRIVATE_NETWORK", "1");
    let (base_url, request_rx) = spawn_identity_server();
    let context = Arc::new(BrowserContext::with_storage_and_network(
        "identity-consistency".to_owned(),
        None,
        false,
        None,
        None,
        true,
    ));
    let mut page = Page::new("identity-page".to_owned(), context);
    page.navigate(&base_url).await.unwrap();
    page.settle(250).await;

    assert_eq!(page.evaluate(identity_expression()), identity_value());
    assert_eq!(page.frame_urls().len(), 1);
    assert_eq!(
        page.evaluate_in_frame(0, identity_expression()).unwrap(),
        identity_value()
    );

    let worker_source = format!(
        "onmessage = function() {{ postMessage(JSON.stringify({{ua: navigator.userAgent, platform: navigator.platform, uaPlatform: navigator.userAgentData.platform}})); }}"
    );
    assert_eq!(
        page.evaluate(&format!(
            "(function() {{ globalThis.__workerIdentity = null; var url = URL.createObjectURL(new Blob([{worker_source:?}], {{type:'application/javascript'}})); var worker = new Worker(url); worker.onmessage = function(event) {{ globalThis.__workerIdentity = JSON.parse(event.data); worker.terminate(); URL.revokeObjectURL(url); }}; worker.postMessage(1); return true; }})()"
        )),
        json!(true)
    );
    for _ in 0..10 {
        page.settle(50).await;
        if page.evaluate("globalThis.__workerIdentity") == identity_value() {
            break;
        }
    }
    assert_eq!(page.evaluate("globalThis.__workerIdentity"), identity_value());

    let mut requests = vec![request_rx.recv_timeout(Duration::from_secs(5)).unwrap()];
    requests.push(request_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    requests.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(requests[0].0, "/");
    assert_eq!(requests[1].0, "/frame.html");
    assert!(requests
        .iter()
        .all(|(_, user_agent)| user_agent == BROWSER_USER_AGENT));
}

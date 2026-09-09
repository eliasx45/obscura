use std::collections::HashMap;
use std::sync::Arc;

use obscura_browser::{BrowserContext, Page};
use serde_json::json;
use url::Url;

#[tokio::test(flavor = "current_thread")]
async fn contexts_do_not_share_cookies_or_document_state() {
    let context_a = Arc::new(BrowserContext::with_storage_and_network(
        "context-a".to_owned(),
        None,
        false,
        None,
        None,
        true,
    ));
    let context_b = Arc::new(BrowserContext::with_storage_and_network(
        "context-b".to_owned(),
        Some("http://proxy-b.invalid:8080".to_owned()),
        false,
        None,
        None,
        false,
    ));

    let context_a = context_a;
    context_a
        .http_client
        .set_extra_headers(HashMap::from([("X-Context".to_owned(), "a".to_owned())]))
        .await;
    assert_eq!(context_a.proxy_url, None);
    assert_eq!(context_b.proxy_url.as_deref(), Some("http://proxy-b.invalid:8080"));
    assert_eq!(context_a.allow_private_network, true);
    assert_eq!(context_b.allow_private_network, false);
    assert_eq!(
        context_a.http_client.extra_headers.read().await.get("X-Context"),
        Some(&"a".to_owned())
    );
    assert!(context_b.http_client.extra_headers.read().await.is_empty());

    let origin = Url::parse("https://example.com/").unwrap();
    context_a.cookie_jar.set_cookie("sid=a", &origin);

    let mut page_a = Page::new("page-a".to_owned(), context_a.clone());
    let mut page_b = Page::new("page-b".to_owned(), context_b.clone());
    page_a.navigate("about:blank").await.unwrap();
    page_b.navigate("about:blank").await.unwrap();

    let write_result = page_a.evaluate(
        r#"(function(){
            globalThis.__owner = 'a';
            document.title = 'context-a';
            localStorage.setItem('owner', 'a');
            sessionStorage.setItem('owner', 'a');
            return JSON.stringify({owner: globalThis.__owner, title: document.title,
                local: localStorage.getItem('owner'), session: sessionStorage.getItem('owner')});
        })()"#,
    );
    assert_eq!(
        write_result,
        json!(r#"{"owner":"a","title":"context-a","local":"a","session":"a"}"#),
        "context A's initial storage/document write must succeed before leakage checks",
    );
    let page_b_state = page_b
        .evaluate(
            r#"JSON.stringify({owner: globalThis.__owner || null, title: document.title,
                local: localStorage.getItem('owner'), session: sessionStorage.getItem('owner')})"#,
        );

    assert_eq!(
        page_b_state,
        json!("{\"owner\":null,\"title\":\"\",\"local\":null,\"session\":null}"),
    );
    assert_eq!(context_a.cookie_jar.get_all_cookies().len(), 1);
    assert!(context_b.cookie_jar.get_all_cookies().is_empty());

    let worker_script = "onmessage = function(event) { postMessage(event.data); }";
    for (page, marker) in [(&mut page_a, "a"), (&mut page_b, "b")] {
        page.evaluate(&format!(
            "(function(){{ var url = URL.createObjectURL(new Blob([{script:?}], {{type:'application/javascript'}})); var worker = new Worker(url); worker.onmessage = function(event){{ globalThis.__workerReply = event.data; worker.terminate(); URL.revokeObjectURL(url); }}; worker.postMessage('{marker}'); }})()",
            script = worker_script,
            marker = marker,
        ));
    }
    page_a.settle(250).await;
    page_b.settle(250).await;
    assert_eq!(page_a.evaluate("globalThis.__workerReply"), json!("a"));
    assert_eq!(page_b.evaluate("globalThis.__workerReply"), json!("b"));

    let persistent = BrowserContext::with_storage_and_network(
        "persistent".to_owned(),
        Some("http://proxy-a.invalid:8080".to_owned()),
        false,
        None,
        None,
        true,
    );
    persistent.cookie_jar.set_cookie("sid=persistent", &origin);
    let copied = persistent.isolated_copy("copied".to_owned(), true);
    let incognito = persistent.isolated_copy("incognito".to_owned(), false);
    assert_eq!(copied.cookie_jar.get_all_cookies().len(), 1);
    assert!(incognito.cookie_jar.get_all_cookies().is_empty());
    assert_eq!(copied.proxy_url, persistent.proxy_url);

    let storage_dir = std::env::temp_dir().join(format!(
        "obscura-context-isolation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let disk = BrowserContext::with_storage_and_network(
        "disk".to_owned(),
        None,
        false,
        None,
        Some(storage_dir.clone()),
        true,
    );
    disk.cookie_jar.set_cookie("sid=disk", &origin);
    disk.save_cookies();
    let reloaded = BrowserContext::with_storage("reloaded".to_owned(), Some(storage_dir.clone()));
    assert_eq!(reloaded.cookie_jar.get_all_cookies().len(), 1);
    std::fs::remove_dir_all(storage_dir).unwrap();
}

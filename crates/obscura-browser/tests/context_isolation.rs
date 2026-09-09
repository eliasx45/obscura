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
        None,
        false,
        None,
        None,
        true,
    ));

    let origin = Url::parse("https://example.com/").unwrap();
    context_a.cookie_jar.set_cookie("sid=a", &origin);

    let mut page_a = Page::new("page-a".to_owned(), context_a.clone());
    let mut page_b = Page::new("page-b".to_owned(), context_b.clone());
    page_a.navigate("about:blank").await.unwrap();
    page_b.navigate("about:blank").await.unwrap();

    page_a
        .evaluate("globalThis.__owner = 'a'; document.title = 'context-a'");
    let page_b_state = page_b
        .evaluate("JSON.stringify({owner: globalThis.__owner || null, title: document.title})");

    assert_eq!(page_b_state, json!("{\"owner\":null,\"title\":\"\"}"));
    assert_eq!(context_a.cookie_jar.get_all_cookies().len(), 1);
    assert!(context_b.cookie_jar.get_all_cookies().is_empty());
}

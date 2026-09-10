use obscura_cdp::dispatch::{dispatch, CdpContext};
use obscura_cdp::types::CdpRequest;
use serde_json::{json, Value};

async fn cdp(
    ctx: &mut CdpContext,
    id: u64,
    method: &str,
    params: Value,
    session_id: Option<&str>,
) -> obscura_cdp::types::CdpResponse {
    dispatch(&CdpRequest {
        id,
        method: method.to_string(),
        params,
        session_id: session_id.map(str::to_string),
    }, ctx).await
}

async fn create_and_attach(ctx: &mut CdpContext, url: &str, id: u64) -> (String, String) {
    let created = cdp(ctx, id, "Target.createTarget", json!({"url": url}), None).await;
    assert!(created.error.is_none(), "createTarget failed: {:?}", created.error);
    let target = created.result.unwrap()["targetId"].as_str().unwrap().to_string();
    let attached = cdp(
        ctx, id + 1, "Target.attachToTarget",
        json!({"targetId": target, "flatten": true}), None,
    ).await;
    let session = attached.result.unwrap()["sessionId"].as_str().unwrap().to_string();
    (target, session)
}

async fn create_context_page(ctx: &mut CdpContext, id: u64) -> (String, String, String) {
    let created = cdp(ctx, id, "Target.createBrowserContext", json!({}), None).await;
    let context = created.result.unwrap()["browserContextId"]
        .as_str()
        .unwrap()
        .to_string();
    let target = cdp(
        ctx,
        id + 1,
        "Target.createTarget",
        json!({"url": "about:blank", "browserContextId": context}),
        None,
    )
    .await;
    let target_id = target.result.unwrap()["targetId"].as_str().unwrap().to_string();
    let attached = cdp(
        ctx,
        id + 2,
        "Target.attachToTarget",
        json!({"targetId": target_id, "flatten": true}),
        None,
    )
    .await;
    let session = attached.result.unwrap()["sessionId"].as_str().unwrap().to_string();
    (context, target_id, session)
}

async fn evaluate_value(
    ctx: &mut CdpContext,
    id: u64,
    session: &str,
    expression: &str,
) -> Value {
    let response = cdp(
        ctx,
        id,
        "Runtime.evaluate",
        json!({"expression": expression, "returnByValue": true}),
        Some(session),
    )
    .await;
    assert!(response.error.is_none(), "evaluation failed: {:?}", response.error);
    response.result.unwrap()["result"]["value"].clone()
}

fn latest_default_context(ctx: &CdpContext, session: &str) -> (i64, String, String) {
    let context = &ctx.pending_events.iter().rev().find(|event| {
        event.method == "Runtime.executionContextCreated"
            && event.session_id.as_deref() == Some(session)
            && event.params["context"]["auxData"]["isDefault"] == true
    }).expect("missing default context").params["context"];
    (
        context["id"].as_i64().unwrap(),
        context["uniqueId"].as_str().unwrap().to_string(),
        context["origin"].as_str().unwrap().to_string(),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn nonblank_create_target_exposes_only_the_committed_document_context() {
    let mut ctx = CdpContext::new();
    let (_, session) = create_and_attach(
        &mut ctx, "data:text/html,<title>committed</title>", 1,
    ).await;
    ctx.pending_events.clear();
    cdp(&mut ctx, 3, "Runtime.enable", json!({}), Some(&session)).await;

    let (id, _, origin) = latest_default_context(&ctx, &session);
    assert_eq!(id, 1);
    assert!(origin.starts_with("data:text/html,"));
    assert!(!ctx.pending_events.iter().any(|event| {
        event.method == "Runtime.executionContextCreated"
            && event.params["context"]["origin"] == "about:blank"
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn default_context_identity_is_page_owned_for_id_and_unique_id() {
    let mut ctx = CdpContext::new();
    let (_, first) = create_and_attach(&mut ctx, "about:blank", 1).await;
    let (_, second) = create_and_attach(&mut ctx, "about:blank", 10).await;
    cdp(&mut ctx, 20, "Runtime.enable", json!({}), Some(&first)).await;
    let (id, unique_id, _) = latest_default_context(&ctx, &first);

    let foreign_id = cdp(
        &mut ctx, 21, "Runtime.evaluate",
        json!({"expression": "1", "contextId": id}), Some(&second),
    ).await;
    assert!(foreign_id.error.unwrap().message.contains("Cannot find context"));
    let foreign_unique = cdp(
        &mut ctx, 22, "Runtime.evaluate",
        json!({"expression": "1", "uniqueContextId": unique_id.clone()}), Some(&second),
    ).await;
    assert!(foreign_unique.error.unwrap().message.contains("Cannot find context"));

    let foreign_call = cdp(
        &mut ctx, 23, "Runtime.callFunctionOn",
        json!({
            "functionDeclaration": "() => 1",
            "executionContextId": id,
            "returnByValue": true,
        }),
        Some(&second),
    ).await;
    assert!(foreign_call.error.unwrap().message.contains("Cannot find context"));

    cdp(
        &mut ctx, 24, "Page.navigate",
        json!({"url": "data:text/html,<p>replacement</p>", "waitUntil": "load"}),
        Some(&first),
    ).await;
    for (request_id, params) in [
        (25, json!({"expression": "1", "uniqueContextId": unique_id})),
        (26, json!({
            "functionDeclaration": "() => 1",
            "executionContextId": id,
            "returnByValue": true,
        })),
    ] {
        let method = if request_id == 25 { "Runtime.evaluate" } else { "Runtime.callFunctionOn" };
        let stale = cdp(&mut ctx, request_id, method, params, Some(&first)).await;
        assert!(stale.error.unwrap().message.contains("Cannot find context"));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn navigating_one_page_preserves_the_other_pages_isolated_context() {
    let mut ctx = CdpContext::new();
    let (_, first) = create_and_attach(&mut ctx, "about:blank", 1).await;
    let (_, second) = create_and_attach(&mut ctx, "about:blank", 10).await;
    for (id, session) in [(20, &first), (21, &second)] {
        cdp(&mut ctx, id, "Runtime.enable", json!({}), Some(session)).await;
    }
    let first_isolated = cdp(
        &mut ctx, 30, "Page.createIsolatedWorld",
        json!({"worldName": "first-utility"}), Some(&first),
    ).await.result.unwrap()["executionContextId"].as_i64().unwrap();
    let second_isolated = cdp(
        &mut ctx, 31, "Page.createIsolatedWorld",
        json!({"worldName": "second-utility"}), Some(&second),
    ).await.result.unwrap()["executionContextId"].as_i64().unwrap();

    cdp(
        &mut ctx, 32, "Page.navigate",
        json!({"url": "data:text/html,<p>first replacement</p>", "waitUntil": "load"}),
        Some(&first),
    ).await;

    let second_still_routes = cdp(
        &mut ctx, 33, "Runtime.evaluate",
        json!({"expression": "1", "contextId": second_isolated}), Some(&second),
    ).await;
    assert!(second_still_routes.error.is_none());
    let first_is_stale = cdp(
        &mut ctx, 34, "Runtime.evaluate",
        json!({"expression": "1", "contextId": first_isolated}), Some(&first),
    ).await;
    assert!(first_is_stale.error.unwrap().message.contains("Cannot find context"));
}

#[tokio::test(flavor = "current_thread")]
async fn live_contexts_keep_state_after_later_context_creation_and_disposal() {
    let mut ctx = CdpContext::new();
    let (context_a, page_a, session_a) = create_context_page(&mut ctx, 1).await;
    let (context_b, page_b, session_b) = create_context_page(&mut ctx, 10).await;

    for (id, session) in [(20, &session_a), (21, &session_b)] {
        let enabled = cdp(&mut ctx, id, "Runtime.enable", json!({}), Some(session)).await;
        assert!(enabled.error.is_none());
    }

    for (id, session, marker) in [(30, &session_a, "A"), (31, &session_b, "B")] {
        let cookie = cdp(
            &mut ctx,
            id,
            "Network.setCookie",
            json!({
                "name": "owner",
                "value": marker,
                "url": "https://example.test/"
            }),
            Some(session),
        )
        .await;
        assert!(cookie.error.is_none(), "cookie write failed: {:?}", cookie.error);

        let written = evaluate_value(
            &mut ctx,
            id + 1,
            session,
            &format!(
                r#"(function() {{
                    const marker = '{marker}';
                    globalThis.__owner = marker;
                    globalThis.__closure = () => marker;
                    globalThis.__eventReply = null;
                    document.body.addEventListener('click', () => {{
                        globalThis.__eventReply = globalThis.__closure();
                    }});
                    document.body.dispatchEvent(new Event('click'));
                    localStorage.setItem('owner', marker);
                    sessionStorage.setItem('owner', marker);
                    const workerUrl = URL.createObjectURL(new Blob([
                        "postMessage('{marker}')"
                    ], {{type: 'application/javascript'}}));
                    globalThis.__worker = new Worker(workerUrl);
                    globalThis.__worker.onmessage = event => {{
                        globalThis.__workerReply = event.data;
                        globalThis.__worker.terminate();
                        URL.revokeObjectURL(workerUrl);
                    }};
                    globalThis.__worker.postMessage(marker);
                    return JSON.stringify({{
                        owner: globalThis.__owner,
                        closure: globalThis.__closure(),
                        event: globalThis.__eventReply,
                        local: localStorage.getItem('owner'),
                        session: sessionStorage.getItem('owner')
                    }});
                }})()"#
            ),
        )
        .await;
        let written: Value = serde_json::from_str(written.as_str().unwrap()).unwrap();
        assert_eq!(written["owner"], marker);
        assert_eq!(written["closure"], marker);
        assert_eq!(written["event"], marker);
        assert_eq!(written["local"], marker);
        assert_eq!(written["session"], marker);
    }

    for page_id in [&page_a, &page_b] {
        ctx.get_page_mut(page_id)
            .expect("page exists")
            .settle(250)
            .await;
    }

    let (context_c, page_c, session_c) = create_context_page(&mut ctx, 40).await;
    let c_state = evaluate_value(&mut ctx, 43, &session_c, "JSON.stringify({owner: globalThis.__owner || null, local: localStorage.getItem('owner'), session: sessionStorage.getItem('owner')})").await;
    assert_eq!(c_state, r#"{"owner":null,"local":null,"session":null}"#);

    for (id, session, marker) in [(50, &session_a, "A"), (60, &session_b, "B")] {
        let state = evaluate_value(
            &mut ctx,
            id,
            session,
            "JSON.stringify({owner: globalThis.__owner, closure: globalThis.__closure(), event: globalThis.__eventReply, worker: globalThis.__workerReply, local: localStorage.getItem('owner'), session: sessionStorage.getItem('owner')})",
        )
        .await;
        let state: Value = serde_json::from_str(state.as_str().unwrap()).unwrap();
        assert_eq!(state["owner"], marker);
        assert_eq!(state["closure"], marker);
        assert_eq!(state["event"], marker);
        assert_eq!(state["worker"], marker);
        assert_eq!(state["local"], marker);
        assert_eq!(state["session"], marker);

        let cookies = cdp(
            &mut ctx,
            id + 1,
            "Network.getCookies",
            json!({"urls": ["https://example.test/"]}),
            Some(session),
        )
        .await;
        assert!(cookies.error.is_none());
        assert_eq!(cookies.result.unwrap()["cookies"][0]["value"], marker);
    }

    cdp(
        &mut ctx,
        70,
        "Target.disposeBrowserContext",
        json!({"browserContextId": context_c}),
        None,
    )
    .await;
    assert!(ctx.get_page(&page_c).is_none());
    assert!(ctx.get_page(&page_a).is_some());
    assert!(ctx.get_page(&page_b).is_some());

    // Re-evaluate after disposing a later context. This is the lifecycle edge
    // that the benchmark previously missed when it only checked immediate
    // writes before post-batch evaluation.
    assert_eq!(evaluate_value(&mut ctx, 71, &session_a, "globalThis.__closure()").await, "A");
    assert_eq!(evaluate_value(&mut ctx, 72, &session_b, "globalThis.__closure()").await, "B");

    cdp(
        &mut ctx,
        80,
        "Target.disposeBrowserContext",
        json!({"browserContextId": context_a}),
        None,
    )
    .await;
    assert!(ctx.get_page(&page_a).is_none());
    assert!(ctx.get_page(&page_b).is_some());
    cdp(
        &mut ctx,
        81,
        "Target.disposeBrowserContext",
        json!({"browserContextId": context_b}),
        None,
    )
    .await;
    assert!(ctx.pages.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn attached_isolated_context_ids_share_the_current_page_global_for_now() {
    let mut ctx = CdpContext::new();
    let (_, session) = create_and_attach(&mut ctx, "about:blank", 1).await;
    cdp(
        &mut ctx, 3, "Runtime.evaluate",
        json!({"expression": "globalThis.realmProbe = 'default'"}), Some(&session),
    ).await;
    let isolated = cdp(
        &mut ctx, 4, "Page.createIsolatedWorld",
        json!({"worldName": "bookkeeping-only"}), Some(&session),
    ).await.result.unwrap()["executionContextId"].as_i64().unwrap();
    let isolated_response = cdp(
        &mut ctx, 5, "Runtime.evaluate",
        json!({
            "expression": "(function(){ globalThis.realmProbe += '-isolated'; return globalThis.realmProbe; })()",
            "contextId": isolated,
            "returnByValue": true,
        }),
        Some(&session),
    ).await;
    assert!(isolated_response.error.is_none(), "isolated evaluate failed: {:?}", isolated_response.error);
    let isolated_value = isolated_response.result.unwrap();
    assert_eq!(isolated_value["result"]["value"], "default-isolated");
    let default_value = cdp(
        &mut ctx, 6, "Runtime.evaluate",
        json!({"expression": "globalThis.realmProbe", "returnByValue": true}),
        Some(&session),
    ).await.result.unwrap();
    assert_eq!(default_value["result"]["value"], "default-isolated");
}

#[tokio::test(flavor = "current_thread")]
async fn navigation_context_events_preserve_order_for_every_runtime_attachment() {
    let mut ctx = CdpContext::new();
    let (target, first) = create_and_attach(&mut ctx, "about:blank", 1).await;
    let attached = cdp(
        &mut ctx, 3, "Target.attachToTarget",
        json!({"targetId": target, "flatten": true}), None,
    ).await;
    let second = attached.result.unwrap()["sessionId"].as_str().unwrap().to_string();
    for (id, session) in [(10, &first), (11, &second)] {
        cdp(&mut ctx, id, "Runtime.enable", json!({}), Some(session)).await;
    }
    cdp(
        &mut ctx, 12, "Page.createIsolatedWorld",
        json!({"worldName": "utility"}), Some(&first),
    ).await;
    ctx.pending_events.clear();

    cdp(
        &mut ctx, 13, "Page.navigate",
        json!({"url": "data:text/html,<p>replacement</p>", "waitUntil": "load"}),
        Some(&first),
    ).await;

    let first_methods = ctx.pending_events.iter().filter(|event| {
        event.session_id.as_deref() == Some(first.as_str())
            && (event.method == "Page.lifecycleEvent"
                || event.method == "Page.frameNavigated"
                || event.method.starts_with("Runtime.executionContext"))
    }).map(|event| {
        if event.method == "Page.lifecycleEvent" {
            format!("{}:{}", event.method, event.params["name"].as_str().unwrap_or(""))
        } else {
            event.method.clone()
        }
    }).collect::<Vec<_>>();
    assert_eq!(&first_methods[..4], [
        "Page.lifecycleEvent:init",
        "Runtime.executionContextsCleared",
        "Page.frameNavigated",
        "Runtime.executionContextCreated",
    ]);
    assert_eq!(first_methods.iter().filter(|method| {
        method.as_str() == "Runtime.executionContextCreated"
    }).count(), 2);

    let second_methods = ctx.pending_events.iter().filter(|event| {
        event.session_id.as_deref() == Some(second.as_str())
            && event.method.starts_with("Runtime.executionContext")
    }).map(|event| event.method.as_str()).collect::<Vec<_>>();
    assert_eq!(second_methods, [
        "Runtime.executionContextsCleared",
        "Runtime.executionContextCreated",
        "Runtime.executionContextCreated",
    ]);
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_and_binding_events_use_the_owning_pages_default_context() {
    let mut ctx = CdpContext::new();
    let (_, first) = create_and_attach(&mut ctx, "about:blank", 1).await;
    let (_, second) = create_and_attach(&mut ctx, "about:blank", 10).await;
    for (id, session) in [(20, &first), (21, &second)] {
        cdp(&mut ctx, id, "Runtime.enable", json!({}), Some(session)).await;
    }
    let (first_context, _, _) = latest_default_context(&ctx, &first);
    cdp(
        &mut ctx,
        22,
        "Runtime.addBinding",
        json!({"name": "probe"}),
        Some(&second),
    ).await;
    ctx.pending_events.clear();

    cdp(
        &mut ctx,
        23,
        "Page.navigate",
        json!({
            "url": "data:text/html,<script>console.log('owned');probe('bound')</script>",
            "waitUntil": "load",
        }),
        Some(&second),
    ).await;
    let (second_context, _, _) = latest_default_context(&ctx, &second);
    assert_ne!(first_context, second_context);

    let console = ctx.pending_events.iter().find(|event| {
        event.method == "Runtime.consoleAPICalled"
            && event.session_id.as_deref() == Some(second.as_str())
    }).expect("missing console event");
    assert_eq!(console.params["executionContextId"], second_context);
    let binding = ctx.pending_events.iter().find(|event| {
        event.method == "Runtime.bindingCalled"
            && event.session_id.as_deref() == Some(second.as_str())
    }).expect("missing binding event");
    assert_eq!(binding.params["executionContextId"], second_context);
}

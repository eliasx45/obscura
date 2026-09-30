//! End-to-end CDP contract used by DOM-agent clients such as Browser Use.
//!
//! This intentionally tests the client workflow, not a detector or a live
//! account: discover the target, navigate, refresh target metadata, build an
//! element index, focus a field, type into it, and read the result back.

use obscura_cdp::dispatch::{dispatch, CdpContext};
use obscura_cdp::types::CdpRequest;
use serde_json::{json, Value};

async fn cdp(
    ctx: &mut CdpContext,
    id: u64,
    method: &str,
    params: Value,
    session_id: Option<&str>,
) -> Value {
    let response = dispatch(
        &CdpRequest {
            id,
            method: method.to_string(),
            params,
            session_id: session_id.map(str::to_string),
        },
        ctx,
    )
    .await;
    assert!(
        response.error.is_none(),
        "CDP {method} failed: {:?}",
        response.error
    );
    response.result.unwrap_or_else(|| json!({}))
}

#[tokio::test(flavor = "current_thread")]
async fn browser_use_dom_agent_workflow_has_a_coherent_cdp_contract() {
    let mut ctx = CdpContext::new();
    let created = cdp(
        &mut ctx,
        1,
        "Target.createTarget",
        json!({"url": "about:blank"}),
        None,
    )
    .await;
    let target_id = created["targetId"].as_str().unwrap().to_string();

    let attached = cdp(
        &mut ctx,
        2,
        "Target.attachToTarget",
        json!({"targetId": target_id, "flatten": true}),
        None,
    )
    .await;
    let session_id = attached["sessionId"].as_str().unwrap().to_string();

    for (id, method) in [
        (3, "Page.enable"),
        (4, "Runtime.enable"),
        (5, "DOM.enable"),
        (6, "DOMSnapshot.enable"),
    ] {
        cdp(&mut ctx, id, method, json!({}), Some(&session_id)).await;
    }

    let url =
        "data:text/html,<main><label>Name</label><input id=name><button id=go>Go</button></main>";
    cdp(
        &mut ctx,
        7,
        "Page.navigate",
        json!({"url": url, "waitUntil": "load"}),
        Some(&session_id),
    )
    .await;

    let target_info = ctx
        .pending_events
        .iter()
        .rev()
        .find(|event| event.method == "Target.targetInfoChanged")
        .expect("navigation must refresh the target metadata");
    assert!(target_info.session_id.is_none());
    assert_eq!(target_info.params["targetInfo"]["targetId"], target_id);
    assert_eq!(target_info.params["targetInfo"]["url"], url);
    assert_eq!(target_info.params["targetInfo"]["title"], "");

    let document = cdp(
        &mut ctx,
        8,
        "DOM.getDocument",
        json!({"depth": -1}),
        Some(&session_id),
    )
    .await;
    let root_node_id = document["root"]["nodeId"]
        .as_u64()
        .expect("DOM.getDocument must return a root node");
    let input = cdp(
        &mut ctx,
        9,
        "DOM.querySelector",
        json!({"nodeId": root_node_id, "selector": "#name"}),
        Some(&session_id),
    )
    .await;
    let input_node_id = input["nodeId"].as_u64().expect("input node id");
    assert!(input_node_id > 0);

    let snapshot = cdp(
        &mut ctx,
        10,
        "DOMSnapshot.captureSnapshot",
        json!({"computedStyles": []}),
        Some(&session_id),
    )
    .await;
    let snapshot_nodes = &snapshot["documents"][0]["nodes"];
    assert!(snapshot_nodes["backendNodeId"].as_array().is_some());
    assert!(snapshot_nodes["isClickable"]["index"]
        .as_array()
        .is_some_and(|indices| !indices.is_empty()));

    cdp(
        &mut ctx,
        11,
        "DOM.focus",
        json!({"nodeId": input_node_id}),
        Some(&session_id),
    )
    .await;
    cdp(
        &mut ctx,
        12,
        "Input.insertText",
        json!({"text": "Elias"}),
        Some(&session_id),
    )
    .await;

    let value = cdp(
        &mut ctx,
        13,
        "Runtime.evaluate",
        json!({
            "expression": "document.querySelector('#name').value",
            "returnByValue": true
        }),
        Some(&session_id),
    )
    .await;
    assert_eq!(value["result"]["value"], "Elias");
}
